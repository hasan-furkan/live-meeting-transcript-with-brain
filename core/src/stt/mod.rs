//! Speech-to-text: whisper.cpp backend and model download.

mod filter;
mod model;
mod segmenter;
mod vad;
mod whisper;

pub use filter::{initial_prompt, is_hallucination};
pub use model::{download_model, model_file_name, valid_model_name};
pub use segmenter::{SegEvent, Segmenter};
pub use vad::{EarshotVad, Vad};
pub use whisper::{Backend, Transcriber, WhisperTranscriber};

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttError {
    ModelMissing,
    ModelLoadFailed,
    TranscriptionFailed,
    DownloadFailed,
    ChecksumMismatch,
    InvalidModelName,
}

impl fmt::Display for SttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelMissing => write!(f, "model file missing"),
            Self::ModelLoadFailed => write!(f, "model load failed"),
            Self::TranscriptionFailed => write!(f, "transcription failed"),
            Self::DownloadFailed => write!(f, "download failed"),
            Self::ChecksumMismatch => write!(f, "checksum mismatch"),
            Self::InvalidModelName => write!(f, "invalid model name"),
        }
    }
}

impl std::error::Error for SttError {}
