//! System audio capture: 16 kHz mono f32 chunks over an `mpsc` channel.

mod resample;
mod wav;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub use resample::to_16k_mono;
pub use wav::WavSource;

use std::fmt;
use std::sync::mpsc::Sender;

/// Samples per chunk: 100 ms at 16 kHz.
pub(crate) const CHUNK: usize = 1_600;

/// A source of 16 kHz mono f32 audio chunks.
pub trait AudioSource: Send {
    fn start(&mut self, tx: Sender<Vec<f32>>) -> Result<(), CaptureError>;
    fn stop(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureError {
    NoDevice,
    PermissionDenied,
    Unsupported,
    Failed,
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NoDevice => "no audio output device",
            Self::PermissionDenied => "audio capture permission denied",
            Self::Unsupported => "audio capture not supported on this platform",
            Self::Failed => "audio capture failed",
        };
        f.write_str(message)
    }
}

impl std::error::Error for CaptureError {}

/// Capture what the system plays, or the named output device.
pub fn system_source(device: Option<&str>) -> Result<Box<dyn AudioSource>, CaptureError> {
    #[cfg(windows)]
    {
        Ok(Box::new(windows::WindowsLoopback::new(device)))
    }
    #[cfg(target_os = "macos")]
    {
        let _ = device;
        Ok(Box::new(macos::MacSystemAudio::new()))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = device;
        Err(CaptureError::Unsupported)
    }
}

pub fn list_output_devices() -> Vec<String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    match host.output_devices() {
        Ok(devices) => devices.filter_map(|device| device.name().ok()).collect(),
        Err(_) => Vec::new(),
    }
}

/// Buffers resampled samples and emits them in 100 ms chunks.
pub(crate) struct Chunker {
    pending: Vec<f32>,
}

impl Chunker {
    pub(crate) fn new() -> Self {
        Self {
            pending: Vec::with_capacity(CHUNK),
        }
    }

    /// Returns false once the receiver is gone.
    pub(crate) fn push(&mut self, samples: &[f32], tx: &Sender<Vec<f32>>) -> bool {
        self.pending.extend_from_slice(samples);
        while self.pending.len() >= CHUNK {
            let chunk: Vec<f32> = self.pending.drain(..CHUNK).collect();
            if tx.send(chunk).is_err() {
                return false;
            }
        }
        true
    }
}
