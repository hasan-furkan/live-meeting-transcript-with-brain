//! Cuts captured audio into fast-pass windows and final segments.
//! All timing is in samples at 16 kHz; VAD is evaluated on 480-sample frames.

use super::vad::Vad;
use std::collections::VecDeque;

const FRAME: usize = 480; // 30 ms @ 16 kHz
const FAST_INTERVAL: usize = 32_000; // 2 s between fast windows
const FAST_WINDOW: usize = 128_000; // fast pass sees at most 8 s
const MIN_SPEECH_FRAMES: usize = 10; // 300 ms of speech required
const SILENCE_END_FRAMES: usize = 20; // 600 ms of silence ends a segment
const MAX_SEGMENT: usize = 240_000; // 15 s
const PRE_ROLL_FRAMES: usize = 6; // 180 ms kept before the first speech frame
const RING_CAP: usize = 480_000; // 30 s

#[derive(Debug, Clone, PartialEq)]
pub enum SegEvent {
    /// Last <=8 s of audio, for the low-latency name pass.
    Fast(Vec<f32>),
    /// One utterance for the final transcript.
    Final(Vec<f32>),
}

pub struct Segmenter {
    /// Recent input audio, capped at 30 s.
    ring: Vec<f32>,
    /// Samples not yet forming a full VAD frame.
    pending: Vec<f32>,
    total: usize,
    next_fast: usize,
    /// End offsets of speech frames, for counting speech in the fast window.
    speech_ends: VecDeque<usize>,
    segment: Vec<f32>,
    pre_roll: Vec<f32>,
    active: bool,
    speech_frames: usize,
    silence_run: usize,
}

impl Default for Segmenter {
    fn default() -> Self {
        Self::new()
    }
}

impl Segmenter {
    pub fn new() -> Self {
        Self {
            ring: Vec::new(),
            pending: Vec::new(),
            total: 0,
            next_fast: FAST_INTERVAL,
            speech_ends: VecDeque::new(),
            segment: Vec::new(),
            pre_roll: Vec::new(),
            active: false,
            speech_frames: 0,
            silence_run: 0,
        }
    }

    pub fn push(&mut self, samples: &[f32], vad: &mut dyn Vad) -> Vec<SegEvent> {
        let mut events = Vec::new();
        self.pending.extend_from_slice(samples);

        let mut consumed = 0;
        let mut frame = [0.0f32; FRAME];
        while self.pending.len() - consumed >= FRAME {
            frame.copy_from_slice(&self.pending[consumed..consumed + FRAME]);
            let speech = vad.is_speech(&frame);
            if speech {
                self.speech_ends.push_back(self.total + consumed + FRAME);
            }
            self.feed_frame(&frame, speech, &mut events);
            consumed += FRAME;
        }
        self.pending.drain(..consumed);

        self.total += samples.len();
        self.ring.extend_from_slice(samples);
        let excess = self.ring.len().saturating_sub(RING_CAP);
        self.ring.drain(..excess);

        while self.total >= self.next_fast {
            self.next_fast += FAST_INTERVAL;
            let cutoff = self.total.saturating_sub(FAST_WINDOW);
            while self.speech_ends.front().is_some_and(|&end| end <= cutoff) {
                self.speech_ends.pop_front();
            }
            if self.speech_ends.len() >= MIN_SPEECH_FRAMES {
                let start = self.ring.len().saturating_sub(FAST_WINDOW);
                events.push(SegEvent::Fast(self.ring[start..].to_vec()));
            }
        }
        events
    }

    fn feed_frame(&mut self, frame: &[f32], speech: bool, events: &mut Vec<SegEvent>) {
        if !self.active {
            if speech {
                self.segment = std::mem::take(&mut self.pre_roll);
                self.segment.extend_from_slice(frame);
                self.active = true;
                self.speech_frames = 1;
                self.silence_run = 0;
            } else {
                self.pre_roll.extend_from_slice(frame);
                let excess = self.pre_roll.len().saturating_sub(PRE_ROLL_FRAMES * FRAME);
                self.pre_roll.drain(..excess);
            }
            return;
        }

        self.segment.extend_from_slice(frame);
        if speech {
            self.speech_frames += 1;
            self.silence_run = 0;
        } else {
            self.silence_run += 1;
        }

        if self.silence_run >= SILENCE_END_FRAMES {
            self.end_segment(events);
        } else if self.segment.len() >= MAX_SEGMENT {
            self.end_segment(events);
            // Speech continues across the 15 s cut: start a new segment right away.
            self.active = speech;
        }
    }

    fn end_segment(&mut self, events: &mut Vec<SegEvent>) {
        let segment = std::mem::take(&mut self.segment);
        let tail = segment.len().saturating_sub(PRE_ROLL_FRAMES * FRAME);
        self.pre_roll = segment[tail..].to_vec();
        if self.speech_frames >= MIN_SPEECH_FRAMES {
            events.push(SegEvent::Final(segment));
        }
        self.active = false;
        self.speech_frames = 0;
        self.silence_run = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::{SegEvent, Segmenter};
    use crate::stt::vad::Vad;

    const SR: usize = 16_000;
    const CHUNK: usize = 1_600;

    /// Fake VAD: speech when any sample is louder than 0.1.
    struct ThresholdVad;

    impl Vad for ThresholdVad {
        fn is_speech(&mut self, f: &[f32]) -> bool {
            f.iter().any(|s| s.abs() > 0.1)
        }
    }

    fn tone(seconds: usize) -> Vec<f32> {
        (0..seconds * SR)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / SR as f32).sin())
            .collect()
    }

    fn zeros(seconds: usize) -> Vec<f32> {
        vec![0.0; seconds * SR]
    }

    fn run(audio: &[f32]) -> Vec<SegEvent> {
        let mut segmenter = Segmenter::new();
        let mut vad = ThresholdVad;
        let mut events = Vec::new();
        for chunk in audio.chunks(CHUNK) {
            events.extend(segmenter.push(chunk, &mut vad));
        }
        events
    }

    fn finals(events: &[SegEvent]) -> Vec<&Vec<f32>> {
        events
            .iter()
            .filter_map(|e| match e {
                SegEvent::Final(s) => Some(s),
                SegEvent::Fast(_) => None,
            })
            .collect()
    }

    #[test]
    fn silence_emits_nothing() {
        assert!(run(&zeros(60)).is_empty());
    }

    #[test]
    fn noise_below_threshold_emits_nothing() {
        let audio: Vec<f32> = tone(60).iter().map(|s| s * 0.1).collect();
        assert!(run(&audio).is_empty());
    }

    #[test]
    fn speech_then_silence_emits_final() {
        let mut audio = tone(2);
        audio.extend(zeros(1));
        let events = run(&audio);
        let finals = finals(&events);
        assert_eq!(finals.len(), 1);
        let len = finals[0].len();
        assert!(
            (32_000..=32_000 + 3_200 + 9_600).contains(&len),
            "final length {len}"
        );
    }

    #[test]
    fn fast_every_2s_during_speech() {
        let events = run(&tone(10));
        let fast = events
            .iter()
            .filter(|e| matches!(e, SegEvent::Fast(_)))
            .count();
        assert_eq!(fast, 5);
    }

    #[test]
    fn final_capped_at_15s() {
        let events = run(&tone(20));
        let finals = finals(&events);
        assert_eq!(finals[0].len(), 240_000);
    }
}
