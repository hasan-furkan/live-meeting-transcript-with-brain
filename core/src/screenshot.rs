//! In-memory screenshots for the LLM. Nothing is written to disk.

use crate::llm::LlmImage;
use crate::settings::{Region, ScreenshotMode, Settings, WindowTarget};
use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use serde::Serialize;
use std::fmt;
use std::time::Duration;
use ts_rs::TS;

const MAX_WIDTH: u32 = 1_600;
const JPEG_QUALITY: u8 = 80;
/// Smaller windows are tool palettes and helpers, not lecture content.
const MIN_WINDOW_WIDTH: u32 = 200;
const MIN_WINDOW_HEIGHT: u32 = 150;
/// A pixel this dark in all three channels counts as black; alpha is ignored.
const BLANK_LEVEL: u8 = 8;
/// A frame this black is treated as a window that stopped drawing, not as content.
const BLANK_SHARE: f64 = 0.995;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotError {
    NoMonitor,
    NoWindow,
    Minimized,
    Unsupported,
    Failed,
    Blank,
}

impl fmt::Display for ShotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NoMonitor => "no monitor found",
            Self::NoWindow => "the chosen window is not open: choose one in Settings",
            Self::Minimized => {
                "the chosen window is minimized: restore it, it can stay behind other windows"
            }
            Self::Unsupported => "window capture needs macOS 14 or later",
            Self::Failed => "screenshot failed",
            Self::Blank => {
                "the window came back black: the app may have stopped drawing it, see Settings"
            }
        };
        f.write_str(message)
    }
}

impl std::error::Error for ShotError {}

/// What to capture; a running session gets a fresh copy whenever settings change.
#[derive(Clone, Debug, PartialEq)]
pub struct ShotConfig {
    pub mode: ScreenshotMode,
    pub monitor: Option<String>,
    pub region: Option<Region>,
    pub window: Option<WindowTarget>,
}

impl ShotConfig {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            mode: settings.screenshot_mode.clone(),
            monitor: settings.monitor.clone(),
            region: settings.region.clone(),
            window: settings.window.clone(),
        }
    }

    pub fn off() -> Self {
        Self {
            mode: ScreenshotMode::Off,
            monitor: None,
            region: None,
            window: None,
        }
    }
}

pub fn list_monitors() -> Result<Vec<String>, ShotError> {
    let monitors = xcap::Monitor::all().map_err(|_| ShotError::Failed)?;
    Ok(monitors
        .iter()
        .filter_map(|monitor| monitor.name().ok())
        .collect())
}

/// Crop the requested region to the monitor and return the visible part.
pub fn clamp_region(region: Region, monitor_width: u32, monitor_height: u32) -> Option<Region> {
    let x0 = i64::from(region.x).max(0);
    let y0 = i64::from(region.y).max(0);
    let x1 = (i64::from(region.x) + i64::from(region.width)).min(i64::from(monitor_width));
    let y1 = (i64::from(region.y) + i64::from(region.height)).min(i64::from(monitor_height));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(Region {
        x: x0 as i32,
        y: y0 as i32,
        width: (x1 - x0) as u32,
        height: (y1 - y0) as u32,
    })
}

/// Capture the chosen monitor whole and report its size in the units `capture_region` uses.
pub fn capture_monitor_full(monitor: Option<&str>) -> Result<(LlmImage, u32, u32), ShotError> {
    let monitors = xcap::Monitor::all().map_err(|_| ShotError::Failed)?;
    let chosen = pick_monitor(&monitors, monitor).ok_or(ShotError::NoMonitor)?;
    let width = chosen.width().map_err(|_| ShotError::Failed)?;
    let height = chosen.height().map_err(|_| ShotError::Failed)?;
    let image = chosen.capture_image().map_err(|_| ShotError::Failed)?;
    Ok((encode_jpeg(image)?, width, height))
}

fn pick_monitor<'a>(
    monitors: &'a [xcap::Monitor],
    name: Option<&str>,
) -> Option<&'a xcap::Monitor> {
    name.and_then(|name| monitors.iter().find(|m| m.name().is_ok_and(|n| n == name)))
        .or_else(|| monitors.iter().find(|m| m.is_primary().unwrap_or(false)))
        .or_else(|| monitors.first())
}

/// Capture the configured monitor, region or window as a JPEG, in memory.
/// A missing or off-monitor region falls back to the whole monitor.
pub fn capture(cfg: &ShotConfig) -> Result<Option<LlmImage>, ShotError> {
    let image = match cfg.mode {
        ScreenshotMode::Off => return Ok(None),
        ScreenshotMode::Window => {
            let target = cfg.window.as_ref().ok_or(ShotError::NoWindow)?;
            capture_window(target)?
        }
        ScreenshotMode::Region | ScreenshotMode::Full => {
            let monitors = xcap::Monitor::all().map_err(|_| ShotError::Failed)?;
            let chosen =
                pick_monitor(&monitors, cfg.monitor.as_deref()).ok_or(ShotError::NoMonitor)?;
            let region = match (&cfg.mode, cfg.region.clone()) {
                (ScreenshotMode::Region, Some(region)) => {
                    let width = chosen.width().map_err(|_| ShotError::Failed)?;
                    let height = chosen.height().map_err(|_| ShotError::Failed)?;
                    clamp_region(region, width, height)
                }
                _ => None,
            };
            match region {
                Some(region) => chosen.capture_region(
                    region.x as u32,
                    region.y as u32,
                    region.width,
                    region.height,
                ),
                None => chosen.capture_image(),
            }
            .map_err(|_| ShotError::Failed)?
        }
    };
    Ok(Some(encode_jpeg(image)?))
}

/// One capturable window, as the OS reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowInfo {
    pub id: u32,
    pub app: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub pid: u32,
    /// The Settings picker shows it by default: on this desktop and not minimized.
    pub visible: bool,
    /// Rolled up by the OS: it cannot be captured until it is restored.
    pub minimized: bool,
}

fn listable(info: &WindowInfo) -> bool {
    // A minimized window reports a tiny rectangle, so the size check skips it.
    (info.minimized || (info.width >= MIN_WINDOW_WIDTH && info.height >= MIN_WINDOW_HEIGHT))
        && !info.title.trim().is_empty()
        && !info.app.trim().is_empty()
        && info.pid != std::process::id()
}

/// The picker's list: the windows that exist now, plus the one the saved choice resolves to.
#[derive(Serialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct WindowList {
    pub windows: Vec<WindowTarget>,
    pub current: Option<WindowTarget>,
}

/// The Settings workaround for a window that returns black frames.
#[derive(Serialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct BackgroundHint {
    pub text: String,
    pub command: String,
}

const BACKGROUND_HINT_TEXT: &str = "Chromium browsers may stop drawing a window you are not looking at, which gives a black image. Quit the browser completely, then start it with this command. Not tested with Microsoft Teams.";

/// The restart command that keeps a Chromium window drawing while it is behind others.
pub fn background_capture_hint() -> BackgroundHint {
    let command = if cfg!(target_os = "macos") {
        "open -na \"Google Chrome\" --args --disable-backgrounding-occluded-windows --disable-renderer-backgrounding"
    } else if cfg!(target_os = "windows") {
        "chrome.exe --disable-backgrounding-occluded-windows --disable-renderer-backgrounding --disable-features=CalculateNativeWinOcclusion"
    } else {
        "google-chrome --disable-backgrounding-occluded-windows --disable-renderer-backgrounding"
    };
    BackgroundHint {
        text: BACKGROUND_HINT_TEXT.to_string(),
        command: command.to_string(),
    }
}

/// Same id and app first, then exact app and title, then the largest window of the app.
/// Hidden windows are eligible: a window on another desktop can still be captured.
/// A minimized window comes last, since it cannot be captured until it is restored.
pub fn pick_window(windows: &[WindowInfo], target: &WindowTarget) -> Option<usize> {
    let listed = || {
        windows
            .iter()
            .enumerate()
            .filter(|(_, info)| listable(info))
    };
    if let Some(id) = target.id {
        for (index, info) in listed() {
            if info.id == id && info.app == target.app {
                return Some(index);
            }
        }
    }
    for (index, info) in listed() {
        if info.app == target.app && info.title == target.title {
            return Some(index);
        }
    }
    // The last of equally large windows wins, as `max_by_key` would.
    // A minimized window is chosen only when no other window of the app exists.
    let mut best: Option<(usize, u64, bool)> = None;
    for (index, info) in listed() {
        if info.app == target.app {
            let area = u64::from(info.width) * u64::from(info.height);
            let better = match best {
                None => true,
                Some((_, current_area, current_minimized)) => {
                    match (current_minimized, info.minimized) {
                        (true, false) => true,
                        (false, true) => false,
                        _ => area >= current_area,
                    }
                }
            };
            if better {
                best = Some((index, area, info.minimized));
            }
        }
    }
    best.map(|(index, _, _)| index)
}

/// The live window a saved choice resolves to, as it is now, or `None` if it is closed.
pub fn list_windows(
    include_hidden: bool,
    current: Option<&WindowTarget>,
) -> Result<WindowList, ShotError> {
    let infos: Vec<WindowInfo> = os::enumerate()?.into_iter().map(|(info, _)| info).collect();
    Ok(window_list(infos, include_hidden, current))
}

fn window_target(info: &WindowInfo) -> WindowTarget {
    WindowTarget {
        id: Some(info.id),
        app: info.app.clone(),
        title: info.title.clone(),
    }
}

/// The live window a saved choice resolves to, if it is still open.
fn live_window(infos: &[WindowInfo], saved: &WindowTarget) -> Option<WindowTarget> {
    let index = pick_window(infos, saved)?;
    infos.get(index).map(window_target)
}

/// The OS-free part of `list_windows`, so tests need no screen.
pub fn window_list(
    infos: Vec<WindowInfo>,
    include_hidden: bool,
    saved: Option<&WindowTarget>,
) -> WindowList {
    let mut windows: Vec<WindowTarget> = infos
        .iter()
        .filter(|info| listable(info))
        .filter(|info| include_hidden || info.visible)
        .map(window_target)
        .collect();
    let current = saved.and_then(|target| live_window(&infos, target));
    if let Some(resolved) = &current {
        // The user chose it, so it exists: list it even when it sits on another desktop.
        windows.push(resolved.clone());
    }
    windows.sort_by(|a, b| (&a.app, &a.title).cmp(&(&b.app, &b.title)));
    // Same-named windows are both kept; only a repeated id is a duplicate.
    let mut seen = std::collections::HashSet::new();
    windows.retain(|window| seen.insert(window.id));
    WindowList { windows, current }
}

/// Share of pixels whose red, green and blue are all at or below `BLANK_LEVEL`.
/// Every pixel is scanned: thin text on a dark slide must still count as content.
pub fn black_share(image: &RgbaImage) -> f64 {
    let total = u64::from(image.width()) * u64::from(image.height());
    if total == 0 {
        return 1.0;
    }
    let black = image
        .pixels()
        .filter(|pixel| {
            pixel.0[0] <= BLANK_LEVEL && pixel.0[1] <= BLANK_LEVEL && pixel.0[2] <= BLANK_LEVEL
        })
        .count() as u64;
    black as f64 / total as f64
}

fn is_blank(image: &RgbaImage) -> bool {
    black_share(image) >= BLANK_SHARE
}

/// Grab a window frame; a black one is retried once after `wait`, then reported as `Blank`.
fn grab_unless_blank(
    mut grab: impl FnMut() -> Result<RgbaImage, ShotError>,
    wait: Duration,
) -> Result<RgbaImage, ShotError> {
    let first = grab()?;
    if !is_blank(&first) {
        return Ok(first);
    }
    std::thread::sleep(wait);
    let second = grab()?;
    if is_blank(&second) {
        return Err(ShotError::Blank);
    }
    Ok(second)
}

fn capture_window(target: &WindowTarget) -> Result<RgbaImage, ShotError> {
    let windows = os::enumerate()?;
    let infos: Vec<WindowInfo> = windows.iter().map(|(info, _)| info.clone()).collect();
    let index = pick_window(&infos, target).ok_or(ShotError::NoWindow)?;
    let (info, handle) = windows.get(index).ok_or(ShotError::NoWindow)?;
    if info.minimized {
        return Err(ShotError::Minimized);
    }
    grab_unless_blank(|| os::grab(handle), Duration::from_millis(250))
}

/// One raw frame of the window `pick_window` resolves: width, height and black share.
/// For `core/examples/window_probe.rs`; there is no blank check here.
pub fn probe_window(target: &WindowTarget) -> Result<(u32, u32, f64), ShotError> {
    let windows = os::enumerate()?;
    let infos: Vec<WindowInfo> = windows.iter().map(|(info, _)| info.clone()).collect();
    let index = pick_window(&infos, target).ok_or(ShotError::NoWindow)?;
    let (info, handle) = windows.get(index).ok_or(ShotError::NoWindow)?;
    if info.minimized {
        return Err(ShotError::Minimized);
    }
    let image = os::grab(handle)?;
    Ok((image.width(), image.height(), black_share(&image)))
}

#[cfg(not(target_os = "macos"))]
mod os {
    use super::{ShotError, WindowInfo};
    use image::RgbaImage;

    pub fn enumerate() -> Result<Vec<(WindowInfo, xcap::Window)>, ShotError> {
        let windows = xcap::Window::all().map_err(|_| ShotError::Failed)?;
        Ok(windows
            .into_iter()
            .filter_map(|window| {
                // A minimized window cannot be captured; it is reported as such instead.
                let minimized = window.is_minimized().unwrap_or(false);
                let info = WindowInfo {
                    id: window.id().ok()?,
                    app: window.app_name().ok()?,
                    title: window.title().ok()?,
                    width: window.width().ok()?,
                    height: window.height().ok()?,
                    pid: window.pid().ok()?,
                    // xcap reports only windows that exist; it has no hidden ones to offer.
                    visible: !minimized,
                    minimized,
                };
                Some((info, window))
            })
            .collect())
    }

    pub fn grab(window: &xcap::Window) -> Result<RgbaImage, ShotError> {
        window.capture_image().map_err(|_| ShotError::Failed)
    }
}

/// ScreenCaptureKit sees windows on other Spaces and behind full-screen apps; xcap does not.
#[cfg(target_os = "macos")]
mod os {
    use super::{ShotError, WindowInfo};
    use image::RgbaImage;
    use screencapturekit::prelude::*;
    use screencapturekit::screenshot_manager::SCScreenshotManager;

    /// Points to pixels on Retina; `encode_jpeg` scales down to `MAX_WIDTH` anyway.
    const SCALE: f64 = 2.0;

    pub fn enumerate() -> Result<Vec<(WindowInfo, SCWindow)>, ShotError> {
        let content = SCShareableContent::create()
            .with_on_screen_windows_only(false)
            .with_exclude_desktop_windows(true)
            .get()
            .map_err(|_| ShotError::Failed)?;
        Ok(content
            .windows()
            .into_iter()
            // Layer 0 holds normal app windows; menus and overlays sit above it.
            .filter(|window| window.window_layer() == 0)
            .filter_map(|window| {
                let app = window.owning_application()?;
                let frame = window.frame();
                let info = WindowInfo {
                    id: window.window_id(),
                    app: app.application_name(),
                    title: window.title()?,
                    width: frame.width.max(0.0) as u32,
                    height: frame.height.max(0.0) as u32,
                    pid: u32::try_from(app.process_id()).ok()?,
                    // Windows of other Spaces and minimized ones are not on screen.
                    // `is_active` needs the `macos_13_0` feature; core/Cargo.toml names none.
                    visible: window.is_on_screen(),
                    minimized: false,
                };
                Some((info, window))
            })
            .collect())
    }

    pub fn grab(window: &SCWindow) -> Result<RgbaImage, ShotError> {
        // SCScreenshotManager is macOS 14+; calling it on 13 would abort the app.
        if !macos_14_or_later() {
            return Err(ShotError::Unsupported);
        }
        let frame = window.frame();
        let filter = SCContentFilter::create().with_window(window).build();
        let config = SCStreamConfiguration::new()
            .with_width((frame.width * SCALE).max(1.0) as u32)
            .with_height((frame.height * SCALE).max(1.0) as u32);
        let image =
            SCScreenshotManager::capture_image(&filter, &config).map_err(|_| ShotError::Failed)?;
        let width = u32::try_from(image.width()).map_err(|_| ShotError::Failed)?;
        let height = u32::try_from(image.height()).map_err(|_| ShotError::Failed)?;
        let rgba = image.rgba_data().map_err(|_| ShotError::Failed)?;
        RgbaImage::from_raw(width, height, rgba).ok_or(ShotError::Failed)
    }

    fn macos_14_or_later() -> bool {
        static AT_LEAST_14: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *AT_LEAST_14.get_or_init(read_macos_14_or_later)
    }

    fn read_macos_14_or_later() -> bool {
        std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|version| version.trim().split('.').next()?.parse::<u32>().ok())
            .is_some_and(|major| major >= 14)
    }
}

fn encode_jpeg(mut image: RgbaImage) -> Result<LlmImage, ShotError> {
    if image.width() > MAX_WIDTH {
        let height = (u64::from(image.height()) * u64::from(MAX_WIDTH) / u64::from(image.width()))
            .max(1) as u32;
        image = image::imageops::resize(&image, MAX_WIDTH, height, FilterType::Triangle);
    }
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
        .encode_image(&DynamicImage::ImageRgba8(image).into_rgb8())
        .map_err(|_| ShotError::Failed)?;
    Ok(LlmImage {
        media_type: "image/jpeg".into(),
        base64: base64::engine::general_purpose::STANDARD.encode(jpeg),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(x: i32, y: i32, width: u32, height: u32) -> Region {
        Region {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn region_clamped_to_monitor() {
        assert_eq!(
            clamp_region(region(10, 20, 100, 50), 1920, 1080),
            Some(region(10, 20, 100, 50))
        );
        assert_eq!(
            clamp_region(region(-10, 1000, 100, 200), 1920, 1080),
            Some(region(0, 1000, 90, 80))
        );
        assert_eq!(clamp_region(region(2000, 0, 100, 100), 1920, 1080), None);
        assert_eq!(clamp_region(region(0, 0, 0, 10), 1920, 1080), None);
    }

    #[test]
    fn off_mode_captures_nothing() {
        assert!(capture(&ShotConfig::off()).unwrap().is_none());
    }

    #[test]
    fn window_mode_without_target_is_no_window() {
        let cfg = ShotConfig {
            mode: ScreenshotMode::Window,
            ..ShotConfig::off()
        };
        assert!(matches!(capture(&cfg), Err(ShotError::NoWindow)));
    }

    fn info(id: u32, app: &str, title: &str, width: u32, height: u32) -> WindowInfo {
        WindowInfo {
            id,
            app: app.into(),
            title: title.into(),
            width,
            height,
            pid: std::process::id().wrapping_add(1),
            visible: true,
            minimized: false,
        }
    }

    fn hidden(mut window: WindowInfo) -> WindowInfo {
        window.visible = false;
        window
    }

    fn minimized(mut window: WindowInfo) -> WindowInfo {
        window.minimized = true;
        window.visible = false;
        window
    }

    fn target(id: Option<u32>, app: &str, title: &str) -> WindowTarget {
        WindowTarget {
            id,
            app: app.into(),
            title: title.into(),
        }
    }

    #[test]
    fn pick_window_prefers_exact_title_then_largest_of_app() {
        let windows = [
            info(1, "Microsoft Teams", "Chat", 900, 700),
            info(2, "Microsoft Teams", "Meeting | Algorithms", 1200, 800),
            info(3, "Google Chrome", "Python lecture - YouTube", 1400, 900),
        ];
        let chat = target(None, "Microsoft Teams", "Chat");
        let stale = target(None, "Microsoft Teams", "Old title");
        let zoom = target(None, "Zoom", "Meeting");

        assert_eq!(pick_window(&windows, &chat), Some(0));
        assert_eq!(pick_window(&windows, &stale), Some(1));
        assert_eq!(pick_window(&windows, &zoom), None);
    }

    #[test]
    fn pick_window_skips_tiny_untitled_and_own_windows() {
        let mut own = info(4, "Google Chrome", "Mine", 1400, 900);
        own.pid = std::process::id();
        let windows = [
            info(1, "Google Chrome", "", 1400, 900),
            info(2, "Google Chrome", "Tooltip", 120, 40),
            own,
        ];
        let tooltip = target(None, "Google Chrome", "Tooltip");
        let mine = target(None, "Google Chrome", "Mine");

        assert_eq!(pick_window(&windows, &tooltip), None);
        assert_eq!(pick_window(&windows, &mine), None);
    }

    #[test]
    fn pick_window_matches_the_id_before_a_same_titled_window() {
        let windows = [
            info(1, "Google Chrome", "Lecture", 1400, 900),
            info(2, "Google Chrome", "Lecture", 1400, 900),
        ];
        let second = target(Some(2), "Google Chrome", "Lecture");
        // A retitled window (a new tab) is still the same window.
        let retitled = target(Some(2), "Google Chrome", "New tab");

        assert_eq!(pick_window(&windows, &second), Some(1));
        assert_eq!(pick_window(&windows, &retitled), Some(1));
    }

    #[test]
    fn pick_window_ignores_an_id_of_another_app() {
        let windows = [info(5, "Slack", "Meeting", 900, 700)];
        let other_app = target(Some(5), "Zoom", "Meeting");

        assert_eq!(pick_window(&windows, &other_app), None);
    }

    #[test]
    fn pick_window_matches_a_minimized_window_by_id() {
        let windows = [minimized(info(7, "Google Chrome", "Lecture", 1400, 900))];
        // A retitled window (a new tab) is still the same window.
        let retitled = target(Some(7), "Google Chrome", "New tab");

        assert_eq!(pick_window(&windows, &retitled), Some(0));
    }

    #[test]
    fn a_tiny_minimized_window_is_listable() {
        // A minimized window reports a tiny rectangle; that must not drop it.
        let windows = [minimized(info(3, "Zoom", "Meeting", 10, 10))];
        let stale = target(None, "Zoom", "Old title");

        assert!(listable(&windows[0]));
        assert_eq!(pick_window(&windows, &stale), Some(0));
    }

    #[test]
    fn pick_window_prefers_a_visible_window_over_a_larger_minimized_one() {
        let windows = [
            minimized(info(1, "Google Chrome", "Lecture", 1400, 900)),
            info(2, "Google Chrome", "Another tab", 800, 600),
        ];
        let stale = target(None, "Google Chrome", "Old title");

        assert_eq!(pick_window(&windows, &stale), Some(1));
    }

    #[test]
    fn pick_window_falls_back_to_a_minimized_window_when_it_is_alone() {
        let windows = [minimized(info(1, "Google Chrome", "Lecture", 1400, 900))];
        let stale = target(None, "Google Chrome", "Old title");

        assert_eq!(pick_window(&windows, &stale), Some(0));
    }

    #[test]
    fn list_windows_reports_a_closed_window_as_absent() {
        let infos = vec![info(1, "Google Chrome", "Lecture", 1400, 900)];
        let saved = target(Some(99), "Zoom", "Meeting");
        let lecture = target(Some(1), "Google Chrome", "Lecture");

        let list = window_list(infos, false, Some(&saved));
        assert_eq!(list.current, None);
        assert!(!list.windows.contains(&saved));
        assert_eq!(list.windows, vec![lecture]);
    }

    #[test]
    fn list_windows_hides_a_minimized_window_unless_asked() {
        let infos = vec![
            info(1, "Google Chrome", "Lecture", 1400, 900),
            minimized(info(2, "Zoom", "Meeting", 900, 700)),
        ];
        let lecture = target(Some(1), "Google Chrome", "Lecture");
        let meeting = target(Some(2), "Zoom", "Meeting");

        let visible_only = window_list(infos.clone(), false, None);
        assert_eq!(visible_only.windows, vec![lecture.clone()]);

        let all = window_list(infos, true, None);
        assert_eq!(all.windows, vec![lecture, meeting]);
    }

    #[test]
    fn list_windows_hides_other_desktops_unless_asked() {
        let infos = vec![
            info(1, "Google Chrome", "Lecture", 1400, 900),
            hidden(info(2, "Zoom", "Meeting", 900, 700)),
        ];
        let lecture = target(Some(1), "Google Chrome", "Lecture");
        let meeting = target(Some(2), "Zoom", "Meeting");

        let visible_only = window_list(infos.clone(), false, None);
        assert_eq!(visible_only.windows, vec![lecture.clone()]);

        let all = window_list(infos, true, None);
        assert_eq!(all.windows, vec![lecture, meeting]);
    }

    #[test]
    fn list_windows_keeps_a_chosen_hidden_window() {
        let infos = vec![
            info(1, "Google Chrome", "Lecture", 1400, 900),
            hidden(info(2, "Zoom", "Meeting", 900, 700)),
        ];
        let saved = target(Some(2), "Zoom", "Meeting");

        let list = window_list(infos, false, Some(&saved));
        assert_eq!(list.current, Some(saved.clone()));
        assert_eq!(list.windows.len(), 2);
        assert!(list.windows.contains(&saved));
    }

    #[test]
    fn list_windows_keeps_same_named_windows_with_different_ids() {
        let infos = vec![
            info(1, "Google Chrome", "Lecture", 1400, 900),
            info(2, "Google Chrome", "Lecture", 1400, 900),
        ];

        let list = window_list(infos, false, None);
        assert_eq!(list.windows.len(), 2);
        assert_ne!(list.windows[0].id, list.windows[1].id);
    }

    #[test]
    fn list_windows_current_carries_the_live_title_and_id() {
        let infos = vec![info(2, "Google Chrome", "Tab two", 1400, 900)];
        let saved = target(Some(2), "Google Chrome", "Old title");
        let live = target(Some(2), "Google Chrome", "Tab two");

        let list = window_list(infos, false, Some(&saved));
        assert_eq!(list.current, Some(live.clone()));
        assert_eq!(list.windows, vec![live]);
    }

    fn frame(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 255]))
    }

    fn light(image: &mut RgbaImage, x: u32, y: u32) {
        image.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn black_share_counts_every_dark_pixel() {
        assert!((black_share(&frame(10, 10)) - 1.0).abs() < f64::EPSILON);
        let mut one_tenth = frame(10, 10);
        for x in 0..10 {
            light(&mut one_tenth, x, 0);
        }
        assert!((black_share(&one_tenth) - 0.9).abs() < 1e-9);
    }

    #[test]
    fn an_all_black_image_is_blank() {
        assert!(is_blank(&frame(40, 30)));
    }

    #[test]
    fn a_transparent_black_image_is_blank() {
        let image = RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 0]));
        assert!(is_blank(&image));
    }

    #[test]
    fn an_empty_image_is_blank() {
        let image = RgbaImage::new(0, 0);
        assert_eq!(black_share(&image), 1.0);
        assert!(is_blank(&image));
    }

    #[test]
    fn a_dark_grey_image_is_not_blank() {
        let image = RgbaImage::from_pixel(40, 30, image::Rgba([40, 40, 40, 255]));
        assert!(!is_blank(&image));
    }

    #[test]
    fn a_black_frame_with_one_percent_white_is_not_blank() {
        let mut image = frame(100, 100);
        for index in 0..100 {
            light(&mut image, index % 10, index / 10);
        }
        assert!(!is_blank(&image));
    }

    #[test]
    fn a_dark_stage_with_a_lit_strip_and_tiles_is_not_blank() {
        let mut image = frame(100, 100);
        // A lit bottom strip (5%) and five lit tiles (5%) on a black stage.
        for y in 95..100 {
            for x in 0..100 {
                light(&mut image, x, y);
            }
        }
        for tile in 0..5 {
            for y in 0..10 {
                for x in 0..10 {
                    light(&mut image, tile * 10 + x, y);
                }
            }
        }
        assert!((black_share(&image) - 0.9).abs() < 1e-9);
        assert!(!is_blank(&image));
    }

    #[test]
    fn a_black_first_frame_is_retried_and_the_second_is_returned() {
        let black = frame(4, 4);
        let good = RgbaImage::from_pixel(4, 4, image::Rgba([200, 200, 200, 255]));
        let mut calls = 0;
        let mut pending = vec![black, good].into_iter();
        let image = grab_unless_blank(
            || {
                calls += 1;
                pending.next().ok_or(ShotError::Failed)
            },
            Duration::ZERO,
        );
        assert!(image.is_ok());
        assert_eq!(calls, 2);
    }

    #[test]
    fn two_black_frames_give_blank() {
        let mut calls = 0;
        let mut pending = vec![frame(4, 4), frame(4, 4)].into_iter();
        let image = grab_unless_blank(
            || {
                calls += 1;
                pending.next().ok_or(ShotError::Failed)
            },
            Duration::ZERO,
        );
        assert!(matches!(image, Err(ShotError::Blank)));
        assert_eq!(calls, 2);
    }

    #[test]
    fn a_good_first_frame_is_grabbed_once() {
        let mut calls = 0;
        let mut pending = vec![RgbaImage::from_pixel(
            4,
            4,
            image::Rgba([200, 200, 200, 255]),
        )]
        .into_iter();
        let image = grab_unless_blank(
            || {
                calls += 1;
                pending.next().ok_or(ShotError::Failed)
            },
            Duration::ZERO,
        );
        assert!(image.is_ok());
        assert_eq!(calls, 1);
    }

    #[test]
    fn a_grab_error_is_returned_at_once() {
        let mut calls = 0;
        let image = grab_unless_blank(
            || {
                calls += 1;
                Err(ShotError::Failed)
            },
            Duration::ZERO,
        );
        assert!(matches!(image, Err(ShotError::Failed)));
        assert_eq!(calls, 1);
    }
}
