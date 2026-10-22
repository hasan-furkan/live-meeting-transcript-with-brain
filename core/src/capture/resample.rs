//! Downmix to mono and resample to 16 kHz.

/// Downmix by averaging channels, then resample to 16 kHz mono.
///
/// Rates that are exact multiples of 16 kHz use box-filter decimation (also
/// anti-aliases); other rates use linear interpolation.
pub fn to_16k_mono(interleaved: &[f32], channels: u16, rate: u32) -> Vec<f32> {
    let channels = channels.max(1) as usize;
    let mono: Vec<f32> = if channels == 1 {
        interleaved.to_vec()
    } else {
        interleaved
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };
    resample(&mono, rate)
}

fn resample(mono: &[f32], rate: u32) -> Vec<f32> {
    const OUT_RATE: u32 = 16_000;
    if mono.is_empty() || rate == 0 {
        return Vec::new();
    }
    if rate == OUT_RATE {
        return mono.to_vec();
    }
    if rate.is_multiple_of(OUT_RATE) {
        let factor = (rate / OUT_RATE) as usize;
        return mono
            .chunks_exact(factor)
            .map(|group| group.iter().sum::<f32>() / factor as f32)
            .collect();
    }
    let out_len = (mono.len() as u64 * OUT_RATE as u64 / rate as u64) as usize;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * f64::from(rate) / f64::from(OUT_RATE);
            let i0 = pos as usize;
            let frac = (pos - i0 as f64) as f32;
            let a = mono[i0.min(mono.len() - 1)];
            let b = mono[(i0 + 1).min(mono.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::to_16k_mono;
    use std::f32::consts::TAU;

    fn sine(rate: u32, seconds: f32, freq: f32, channels: u16) -> Vec<f32> {
        let frames = (rate as f32 * seconds) as usize;
        let mut out = Vec::with_capacity(frames * channels as usize);
        for i in 0..frames {
            let value = (TAU * freq * i as f32 / rate as f32).sin();
            for _ in 0..channels {
                out.push(value);
            }
        }
        out
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn resample_48k_stereo_to_16k_length() {
        let input = sine(48_000, 1.0, 440.0, 2);
        let out = to_16k_mono(&input, 2, 48_000);
        assert!((out.len() as i64 - 16_000).abs() <= 1, "{}", out.len());
    }

    #[test]
    fn resample_preserves_1khz_tone_energy() {
        let input = sine(48_000, 1.0, 1_000.0, 1);
        let out = to_16k_mono(&input, 1, 48_000);
        let (before, after) = (rms(&input), rms(&out));
        assert!(
            (before - after).abs() / before < 0.10,
            "{before} vs {after}"
        );
    }

    #[test]
    fn resample_16k_mono_returns_input_unchanged() {
        let input = sine(16_000, 0.1, 440.0, 1);
        assert_eq!(to_16k_mono(&input, 1, 16_000), input);
    }

    #[test]
    fn resample_empty_input_is_empty() {
        for rate in [16_000, 44_100, 48_000] {
            assert!(to_16k_mono(&[], 2, rate).is_empty());
        }
    }
}
