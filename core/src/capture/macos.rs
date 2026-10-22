//! macOS system audio through ScreenCaptureKit.

use super::{AudioSource, CaptureError, Chunker, to_16k_mono};
use screencapturekit::AudioBufferList;
use screencapturekit::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
const POLL: std::time::Duration = std::time::Duration::from_millis(100);
const START_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

pub struct MacSystemAudio {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl MacSystemAudio {
    pub fn new() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }
}

impl AudioSource for MacSystemAudio {
    fn start(&mut self, tx: Sender<Vec<f32>>) -> Result<(), CaptureError> {
        if self.handle.is_some() {
            return Err(CaptureError::Failed);
        }
        self.stop.store(false, Ordering::Relaxed);
        let stop = self.stop.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || run(tx, stop, ready_tx));
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
                // Still inside ScreenCaptureKit: signal stop and detach rather
                // than join, which would block until it returns.
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

fn run(tx: Sender<Vec<f32>>, stop: Arc<AtomicBool>, ready: Sender<Result<(), CaptureError>>) {
    let stream = match start_stream(tx, &stop) {
        Ok(stream) => {
            let _ = ready.send(Ok(()));
            stream
        }
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(POLL);
    }
    let _ = stream.stop_capture();
}

fn start_stream(tx: Sender<Vec<f32>>, stop: &Arc<AtomicBool>) -> Result<SCStream, CaptureError> {
    let content = SCShareableContent::get().map_err(map_error)?;
    let display = content
        .displays()
        .into_iter()
        .next()
        .ok_or(CaptureError::NoDevice)?;
    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&[])
        .build();
    let config = SCStreamConfiguration::new()
        .with_width(2)
        .with_height(2)
        .with_minimum_frame_interval(&CMTime::new(1, 1))
        .with_captures_audio(true)
        .with_sample_rate(SAMPLE_RATE as i32)
        .with_channel_count(CHANNELS);
    let mut stream = SCStream::new(&filter, &config);
    let chunker = RefCell::new(Chunker::new());
    let stop = stop.clone();
    stream.add_output_handler(
        move |sample: CMSampleBuffer, of_type: SCStreamOutputType| {
            if of_type != SCStreamOutputType::Audio {
                return;
            }
            let Some(buffers) = sample.audio_buffer_list() else {
                return;
            };
            let (interleaved, channels) = interleaved_f32(&buffers);
            let mono = to_16k_mono(&interleaved, channels, SAMPLE_RATE);
            if !chunker.borrow_mut().push(&mono, &tx) {
                stop.store(true, Ordering::Relaxed);
            }
        },
        SCStreamOutputType::Audio,
    );
    stream.start_capture().map_err(map_error)?;
    Ok(stream)
}

fn map_error(error: SCError) -> CaptureError {
    match error {
        SCError::PermissionDenied(_) => CaptureError::PermissionDenied,
        SCError::NoShareableContent(_) => CaptureError::NoDevice,
        _ => CaptureError::Failed,
    }
}

/// ScreenCaptureKit delivers float32 PCM, either interleaved in one buffer or
/// one buffer per channel.
fn interleaved_f32(buffers: &AudioBufferList) -> (Vec<f32>, u16) {
    let planes: Vec<Vec<f32>> = buffers
        .iter()
        .map(|buffer| {
            buffer
                .data()
                .chunks_exact(4)
                .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                .collect()
        })
        .collect();
    if planes.len() == 1 {
        let channels = buffers.get(0).map_or(1, |buffer| buffer.number_channels);
        (
            planes.into_iter().next().unwrap_or_default(),
            channels.max(1) as u16,
        )
    } else {
        let frames = planes.iter().map(Vec::len).min().unwrap_or(0);
        let mut interleaved = Vec::with_capacity(frames * planes.len());
        for frame in 0..frames {
            for plane in &planes {
                interleaved.push(plane[frame]);
            }
        }
        (interleaved, planes.len() as u16)
    }
}
