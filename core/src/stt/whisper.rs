use super::SttError;
use super::model::valid_model_file_name;
use serde::Serialize;
use std::path::Path;
use ts_rs::TS;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

#[derive(Serialize, TS, Clone, Copy, Debug, PartialEq)]
#[ts(export)]
pub enum Backend {
    Gpu,
    Cpu,
}

pub trait Transcriber: Send {
    fn transcribe(&mut self, pcm16k: &[f32], prompt: Option<&str>) -> Result<String, SttError>;
    fn backend(&self) -> Backend;
}

pub struct WhisperTranscriber {
    state: WhisperState,
    language: Option<String>,
    backend: Backend,
}

impl WhisperTranscriber {
    pub fn load(model_path: &Path, language: &str) -> Result<Self, SttError> {
        let named = model_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(valid_model_file_name);
        if !named {
            return Err(SttError::InvalidModelName);
        }
        if !model_path.is_file() {
            return Err(SttError::ModelMissing);
        }
        // Keeps whisper.cpp/ggml logs off stdout; without a log backend they are dropped.
        whisper_rs::install_logging_hooks();
        let mut params = WhisperContextParameters::new();
        params.use_gpu(true);
        let ctx = WhisperContext::new_with_params(model_path, params)
            .map_err(|_| SttError::ModelLoadFailed)?;
        let state = ctx.create_state().map_err(|_| SttError::ModelLoadFailed)?;
        Ok(Self {
            state,
            language: (language != "auto").then(|| language.to_string()),
            backend: detect_backend(),
        })
    }
}

/// GPU only when the crate was built with a GPU backend and whisper.cpp registered it.
fn detect_backend() -> Backend {
    if cfg!(any(feature = "cuda", feature = "metal")) {
        let info = whisper_rs::print_system_info().to_ascii_uppercase();
        if info.contains("CUDA") || info.contains("METAL") {
            return Backend::Gpu;
        }
    }
    Backend::Cpu
}

impl Transcriber for WhisperTranscriber {
    fn transcribe(&mut self, pcm16k: &[f32], prompt: Option<&str>) -> Result<String, SttError> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_no_context(true);
        params.set_language(self.language.as_deref());
        if let Some(prompt) = prompt.filter(|p| !p.trim().is_empty()) {
            params.set_initial_prompt(prompt);
        }
        self.state
            .full(params, pcm16k)
            .map_err(|_| SttError::TranscriptionFailed)?;
        let mut text = String::new();
        for segment in self.state.as_iter() {
            text.push_str(
                &segment
                    .to_str_lossy()
                    .map_err(|_| SttError::TranscriptionFailed)?,
            );
        }
        Ok(text.trim().to_string())
    }

    fn backend(&self) -> Backend {
        self.backend
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_model_fails() {
        let err = WhisperTranscriber::load(Path::new("ggml-does-not-exist.bin"), "en").err();
        assert_eq!(err, Some(SttError::ModelMissing));
    }

    #[test]
    fn load_rejects_unsafe_model_file_name() {
        let err = WhisperTranscriber::load(Path::new("models/evil.bin"), "en").err();
        assert_eq!(err, Some(SttError::InvalidModelName));
        let err = WhisperTranscriber::load(Path::new("models/ggml-../evil.bin"), "en").err();
        assert_eq!(err, Some(SttError::InvalidModelName));
    }
}
