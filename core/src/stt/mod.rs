//! Speech-to-text: whisper.cpp backend and model download.

mod model;
mod whisper;

pub use model::{download_model, model_file_name};
pub use whisper::{Backend, Transcriber, WhisperTranscriber};

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttError {
    ModelMissing,
    ModelLoadFailed,
    TranscriptionFailed,
    DownloadFailed,
    ChecksumMismatch,
}

impl fmt::Display for SttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelMissing => write!(f, "model file missing"),
            Self::ModelLoadFailed => write!(f, "model load failed"),
            Self::TranscriptionFailed => write!(f, "transcription failed"),
            Self::DownloadFailed => write!(f, "download failed"),
            Self::ChecksumMismatch => write!(f, "checksum mismatch"),
        }
    }
}

impl std::error::Error for SttError {}
