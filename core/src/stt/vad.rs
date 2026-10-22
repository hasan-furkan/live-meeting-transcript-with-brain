/// Voice activity detection over 480-sample (30 ms @ 16 kHz) mono f32 frames.
pub trait Vad: Send {
    fn is_speech(&mut self, frame: &[f32]) -> bool;
}

const EARSHOT_FRAME: usize = 256; // earshot's fixed frame size (16 ms)

/// Earshot-based VAD. Re-chunks our 30 ms frames into earshot's 256-sample frames,
/// keeping the remainder for the next call; a frame counts as speech if any inner
/// earshot frame scores >= 0.5.
pub struct EarshotVad {
    detector: Box<earshot::Detector>,
    pending: Vec<f32>,
}

impl EarshotVad {
    pub fn new() -> Self {
        Self {
            detector: earshot::Detector::default_boxed(),
            pending: Vec::with_capacity(EARSHOT_FRAME * 2),
        }
    }
}

impl Default for EarshotVad {
    fn default() -> Self {
        Self::new()
    }
}

impl Vad for EarshotVad {
    fn is_speech(&mut self, frame: &[f32]) -> bool {
        self.pending.extend_from_slice(frame);
        let mut speech = false;
        let mut consumed = 0;
        while consumed + EARSHOT_FRAME <= self.pending.len() {
            let chunk = &self.pending[consumed..consumed + EARSHOT_FRAME];
            if self.detector.predict_f32(chunk) >= 0.5 {
                speech = true;
            }
            consumed += EARSHOT_FRAME;
        }
        self.pending.drain(..consumed);
        speech
    }
}

#[cfg(test)]
mod tests {
    use super::{EarshotVad, Vad};

    const FRAME: usize = 480;

    fn count_speech(audio: &[f32]) -> usize {
        let mut vad = EarshotVad::new();
        audio.chunks(FRAME).filter(|f| vad.is_speech(f)).count()
    }

    /// Uniform white noise in [-amp, amp] from a fixed LCG, so the test is deterministic.
    fn white_noise(len: usize, amp: f32) -> Vec<f32> {
        let mut seed = 12_345u32;
        (0..len)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((seed >> 8) as f32 / 8_388_608.0 - 1.0) * amp
            })
            .collect()
    }

    #[test]
    fn earshot_silence_and_noise() {
        assert_eq!(count_speech(&vec![0.0; 3 * 16_000]), 0);
        let noise_frames = count_speech(&white_noise(3 * 16_000, 0.02));
        assert!(
            noise_frames < 10,
            "noise produced {noise_frames} speech frames"
        );
    }
}
