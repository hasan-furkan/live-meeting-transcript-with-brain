//! In-memory screenshots for the LLM. Nothing is written to disk.

use crate::llm::LlmImage;
use crate::settings::{Region, ScreenshotMode};
use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use std::fmt;

const MAX_WIDTH: u32 = 1_600;
const JPEG_QUALITY: u8 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotError {
    NoMonitor,
    NoRegion,
    Failed,
}

impl fmt::Display for ShotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NoMonitor => "no monitor found",
            Self::NoRegion => "screenshot region missing",
            Self::Failed => "screenshot failed",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ShotError {}

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

/// Capture the chosen monitor whole and report its physical size as well.
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

/// Capture the configured monitor or region as a JPEG, in memory.
pub fn capture(
    mode: ScreenshotMode,
    monitor: Option<&str>,
    region: Option<Region>,
) -> Result<Option<LlmImage>, ShotError> {
    if mode == ScreenshotMode::Off {
        return Ok(None);
    }
    let monitors = xcap::Monitor::all().map_err(|_| ShotError::Failed)?;
    let chosen = pick_monitor(&monitors, monitor).ok_or(ShotError::NoMonitor)?;
    let image = match mode {
        ScreenshotMode::Region => {
            let region = region.ok_or(ShotError::NoRegion)?;
            let width = chosen.width().map_err(|_| ShotError::Failed)?;
            let height = chosen.height().map_err(|_| ShotError::Failed)?;
            let region = clamp_region(region, width, height).ok_or(ShotError::NoRegion)?;
            chosen.capture_region(
                region.x as u32,
                region.y as u32,
                region.width,
                region.height,
            )
        }
        ScreenshotMode::Off | ScreenshotMode::Full => chosen.capture_image(),
    }
    .map_err(|_| ShotError::Failed)?;
    Ok(Some(encode_jpeg(image)?))
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
        assert!(capture(ScreenshotMode::Off, None, None).unwrap().is_none());
    }
}
