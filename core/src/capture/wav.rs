//! WAV file audio source, used for tests and manual checks.

use super::{AudioSource, CHUNK, CaptureError, to_16k_mono};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::Duration;

pub struct WavSource {
    path: PathBuf,
    realtime: bool,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl WavSource {
    pub fn new(path: PathBuf, realtime: bool) -> Self {
        Self {
            path,
            realtime,
            stop: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }
}

impl AudioSource for WavSource {
    fn start(&mut self, tx: Sender<Vec<f32>>) -> Result<(), CaptureError> {
        if self.handle.is_some() {
            return Err(CaptureError::Failed);
        }
        let samples = read_wav(&self.path)?;
        self.stop.store(false, Ordering::Relaxed);
        let stop = self.stop.clone();
        let realtime = self.realtime;
        self.handle = Some(std::thread::spawn(move || {
            for chunk in samples.chunks(CHUNK) {
                if stop.load(Ordering::Relaxed) || tx.send(chunk.to_vec()).is_err() {
                    return;
                }
                if realtime {
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }));
        Ok(())
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn read_wav(path: &Path) -> Result<Vec<f32>, CaptureError> {
    let mut reader = hound::WavReader::open(path).map_err(|_| CaptureError::Failed)?;
    let spec = reader.spec();
    let samples = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| CaptureError::Failed)?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << u32::from(spec.bits_per_sample.clamp(1, 32) - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|s| s as f32 * scale))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| CaptureError::Failed)?
        }
    };
    Ok(to_16k_mono(&samples, spec.channels, spec.sample_rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tone(path: &Path, rate: u32, channels: u16, frames: usize) {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..frames {
            for _ in 0..channels {
                let value = ((i as f32 * 0.1).sin() * 8_000.0) as i16;
                writer.write_sample(value).unwrap();
            }
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn wav_source_delivers_all_samples() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        write_tone(&path, 16_000, 1, 4_800);
        let mut source = WavSource::new(path, false);
        let (tx, rx) = std::sync::mpsc::channel();
        source.start(tx).unwrap();
        let delivered: usize = rx.iter().map(|chunk| chunk.len()).sum();
        assert_eq!(delivered, 4_800);
        source.stop();
    }
}
