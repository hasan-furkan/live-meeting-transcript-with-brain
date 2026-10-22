//! Windows system audio through WASAPI loopback (cpal).

use super::{AudioSource, CaptureError, Chunker, to_16k_mono};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

const POLL: Duration = Duration::from_secs(2);
const START_TIMEOUT: Duration = Duration::from_secs(5);

pub struct WindowsLoopback {
    device: Option<String>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl WindowsLoopback {
    pub fn new(device: Option<&str>) -> Self {
        Self {
            device: device.map(str::to_owned),
            stop: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }
}

impl AudioSource for WindowsLoopback {
    fn start(&mut self, tx: Sender<Vec<f32>>) -> Result<(), CaptureError> {
        if self.handle.is_some() {
            return Err(CaptureError::Failed);
        }
        self.stop.store(false, Ordering::Relaxed);
        let device = self.device.clone();
        let stop = self.stop.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || run(device, tx, stop, ready_tx));
        match ready_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(())) => {
                self.handle = Some(handle);
                Ok(())
            }
            Ok(Err(error)) => {
                let _ = handle.join();
                Err(error)
            }
            Err(_) => {
                // The thread exits on its own once its current build returns.
                self.stop.store(true, Ordering::Relaxed);
                Err(CaptureError::Failed)
            }
        }
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Owns the stream on a dedicated thread because `cpal::Stream` is not `Send`.
fn run(
    device: Option<String>,
    tx: Sender<Vec<f32>>,
    stop: Arc<AtomicBool>,
    ready: Sender<Result<(), CaptureError>>,
) {
    let failed = Arc::new(AtomicBool::new(false));
    // The first open failure goes back to `start`; later rebuilds retry.
    let (mut stream, mut current) = match build(device.as_deref(), &tx, &stop, &failed) {
        Ok((stream, name)) => {
            let _ = ready.send(Ok(()));
            (Some(stream), Some(name))
        }
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(POLL);
        let broken = failed.swap(false, Ordering::Relaxed);
        let default_changed = device.is_none()
            && current.is_some()
            && default_output_name().is_some_and(|name| Some(name) != current);
        if broken || default_changed {
            stream = None;
            current = None;
        }
        if stream.is_none()
            && let Ok((new_stream, name)) = build(device.as_deref(), &tx, &stop, &failed)
        {
            failed.store(false, Ordering::Relaxed);
            stream = Some(new_stream);
            current = Some(name);
        }
    }
}

fn default_output_name() -> Option<String> {
    cpal::default_host().default_output_device()?.name().ok()
}

fn build(
    device_name: Option<&str>,
    tx: &Sender<Vec<f32>>,
    stop: &Arc<AtomicBool>,
    failed: &Arc<AtomicBool>,
) -> Result<(cpal::Stream, String), CaptureError> {
    let host = cpal::default_host();
    let device = match device_name {
        Some(name) => host
            .output_devices()
            .map_err(|_| CaptureError::Failed)?
            .find(|device| device.name().ok().as_deref() == Some(name))
            .ok_or(CaptureError::NoDevice)?,
        None => host.default_output_device().ok_or(CaptureError::NoDevice)?,
    };
    let name = device.name().map_err(|_| CaptureError::NoDevice)?;
    let supported = device
        .default_output_config()
        .map_err(|_| CaptureError::Failed)?;
    let format = supported.sample_format();
    let config = supported.config();
    let stream = match format {
        SampleFormat::I8 => input::<i8>(&device, &config, tx, stop, failed)?,
        SampleFormat::I16 => input::<i16>(&device, &config, tx, stop, failed)?,
        SampleFormat::I24 => input::<cpal::I24>(&device, &config, tx, stop, failed)?,
        SampleFormat::I32 => input::<i32>(&device, &config, tx, stop, failed)?,
        SampleFormat::I64 => input::<i64>(&device, &config, tx, stop, failed)?,
        SampleFormat::U8 => input::<u8>(&device, &config, tx, stop, failed)?,
        SampleFormat::U16 => input::<u16>(&device, &config, tx, stop, failed)?,
        SampleFormat::U32 => input::<u32>(&device, &config, tx, stop, failed)?,
        SampleFormat::U64 => input::<u64>(&device, &config, tx, stop, failed)?,
        SampleFormat::F32 => input::<f32>(&device, &config, tx, stop, failed)?,
        SampleFormat::F64 => input::<f64>(&device, &config, tx, stop, failed)?,
        _ => return Err(CaptureError::Unsupported),
    };
    stream.play().map_err(|_| CaptureError::Failed)?;
    Ok((stream, name))
}

/// An input stream on an output device records the loopback mix on WASAPI.
fn input<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    tx: &Sender<Vec<f32>>,
    stop: &Arc<AtomicBool>,
    failed: &Arc<AtomicBool>,
) -> Result<cpal::Stream, CaptureError>
where
    T: SizedSample + 'static,
    f32: FromSample<T>,
{
    let channels = config.channels;
    let rate = config.sample_rate.0;
    let tx = tx.clone();
    let stop = stop.clone();
    let failed = failed.clone();
    let mut chunker = Chunker::new();
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let interleaved: Vec<f32> = data
                    .iter()
                    .map(|sample| sample.to_sample::<f32>())
                    .collect();
                let mono = to_16k_mono(&interleaved, channels, rate);
                if !chunker.push(&mono, &tx) {
                    stop.store(true, Ordering::Relaxed);
                }
            },
            move |_| failed.store(true, Ordering::Relaxed),
            None,
        )
        .map_err(|_| CaptureError::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_reports_missing_named_device() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut source = WindowsLoopback::new(Some("lma-test-no-such-output-device"));
        assert_eq!(source.start(tx), Err(CaptureError::NoDevice));
    }
}
