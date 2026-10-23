//! In-memory screenshots for the LLM. Nothing is written to disk.

use crate::llm::LlmImage;
use crate::settings::{Region, ScreenshotMode, Settings, WindowTarget};
use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use std::fmt;

const MAX_WIDTH: u32 = 1_600;
const JPEG_QUALITY: u8 = 80;
/// Smaller windows are tool palettes and helpers, not lecture content.
const MIN_WINDOW_WIDTH: u32 = 200;
const MIN_WINDOW_HEIGHT: u32 = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotError {
    NoMonitor,
    NoWindow,
    Unsupported,
    Failed,
}

impl fmt::Display for ShotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NoMonitor => "no monitor found",
            Self::NoWindow => "window not found: choose one in Settings (closed or minimized?)",
            Self::Unsupported => "window capture needs macOS 14 or later",
            Self::Failed => "screenshot failed",
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
    pub app: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub pid: u32,
}

fn listable(info: &WindowInfo) -> bool {
    info.width >= MIN_WINDOW_WIDTH
        && info.height >= MIN_WINDOW_HEIGHT
        && !info.title.trim().is_empty()
        && !info.app.trim().is_empty()
        && info.pid != std::process::id()
}

/// Exact app and title first; otherwise the largest window of the same app.
pub fn pick_window(windows: &[WindowInfo], target: &WindowTarget) -> Option<usize> {
    let listed = || {
        windows
            .iter()
            .enumerate()
            .filter(|(_, info)| listable(info))
    };
    listed()
        .find(|(_, info)| info.app == target.app && info.title == target.title)
        .or_else(|| {
            listed()
                .filter(|(_, info)| info.app == target.app)
                .max_by_key(|(_, info)| u64::from(info.width) * u64::from(info.height))
        })
        .map(|(index, _)| index)
}

/// Windows for the Settings picker, sorted and without duplicates.
pub fn list_windows() -> Result<Vec<WindowTarget>, ShotError> {
    let mut targets: Vec<WindowTarget> = os::enumerate()?
        .into_iter()
        .filter(|(info, _)| listable(info))
        .map(|(info, _)| WindowTarget {
            app: info.app,
            title: info.title,
        })
        .collect();
    targets.sort_by(|a, b| (&a.app, &a.title).cmp(&(&b.app, &b.title)));
    targets.dedup();
    Ok(targets)
}

fn capture_window(target: &WindowTarget) -> Result<RgbaImage, ShotError> {
    let windows = os::enumerate()?;
    let infos: Vec<WindowInfo> = windows.iter().map(|(info, _)| info.clone()).collect();
    let index = pick_window(&infos, target).ok_or(ShotError::NoWindow)?;
    let (_, handle) = windows.get(index).ok_or(ShotError::NoWindow)?;
    os::grab(handle)
}

#[cfg(not(target_os = "macos"))]
mod os {
    use super::{ShotError, WindowInfo};
    use image::RgbaImage;

    pub fn enumerate() -> Result<Vec<(WindowInfo, xcap::Window)>, ShotError> {
        let windows = xcap::Window::all().map_err(|_| ShotError::Failed)?;
        Ok(windows
            .into_iter()
            // A minimized window captures as a black frame.
            .filter(|window| !window.is_minimized().unwrap_or(true))
            .filter_map(|window| {
                let info = WindowInfo {
                    app: window.app_name().ok()?,
                    title: window.title().ok()?,
                    width: window.width().ok()?,
                    height: window.height().ok()?,
                    pid: window.pid().ok()?,
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
                    app: app.application_name(),
                    title: window.title()?,
                    width: frame.width.max(0.0) as u32,
                    height: frame.height.max(0.0) as u32,
                    pid: u32::try_from(app.process_id()).ok()?,
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

    fn info(app: &str, title: &str, width: u32, height: u32) -> WindowInfo {
        WindowInfo {
            app: app.into(),
            title: title.into(),
            width,
            height,
            pid: std::process::id().wrapping_add(1),
        }
    }

    fn target(app: &str, title: &str) -> WindowTarget {
        WindowTarget {
            app: app.into(),
            title: title.into(),
        }
    }

    #[test]
    fn pick_window_prefers_exact_title_then_largest_of_app() {
        let windows = [
            info("Microsoft Teams", "Chat", 900, 700),
            info("Microsoft Teams", "Meeting | Algorithms", 1200, 800),
            info("Google Chrome", "Python lecture - YouTube", 1400, 900),
        ];
        assert_eq!(
            pick_window(&windows, &target("Microsoft Teams", "Chat")),
            Some(0)
        );
        assert_eq!(
            pick_window(&windows, &target("Microsoft Teams", "Old title")),
            Some(1)
        );
        assert_eq!(pick_window(&windows, &target("Zoom", "Meeting")), None);
    }

    #[test]
    fn pick_window_skips_tiny_untitled_and_own_windows() {
        let mut own = info("Google Chrome", "Mine", 1400, 900);
        own.pid = std::process::id();
        let windows = [
            info("Google Chrome", "", 1400, 900),
            info("Google Chrome", "Tooltip", 120, 40),
            own,
        ];
        assert_eq!(
            pick_window(&windows, &target("Google Chrome", "Tooltip")),
            None
        );
        assert_eq!(
            pick_window(&windows, &target("Google Chrome", "Mine")),
            None
        );
    }
}
