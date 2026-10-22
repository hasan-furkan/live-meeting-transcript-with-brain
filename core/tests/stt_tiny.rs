#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use lma_core::stt::{Transcriber, WhisperTranscriber, download_model, model_file_name};
use std::path::PathBuf;

fn model_dir() -> PathBuf {
    std::env::var_os("LMA_TEST_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap()).join("lma-test-models")
        })
}

#[tokio::test]
#[ignore = "downloads ggml-tiny.bin; needs a GPU build to report the Gpu backend"]
async fn whisper_tiny_transcribes_fixture() {
    let dir = model_dir();
    let path = dir.join(model_file_name("tiny"));
    if !path.is_file() {
        println!("downloading ggml-tiny.bin to {}", dir.display());
        download_model("tiny", &dir, |_, _| {}).await.unwrap();
    }

    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/jordan.wav");
    let mut reader = hound::WavReader::open(fixture).unwrap();
    let spec = reader.spec();
    assert_eq!((spec.sample_rate, spec.channels), (16000, 1));
    let pcm: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();
    assert!(pcm.len() <= 16000 * 10);

    let mut transcriber = WhisperTranscriber::load(&path, "en").unwrap();
    let text = transcriber.transcribe(&pcm, None).unwrap();
    let normalized = lma_core::triggers::normalize(&text);
    println!("backend: {:?}\ntext: {text}", transcriber.backend());

    assert!(normalized.contains("merge sort"), "text was: {text}");
    assert!(
        normalized.contains("jordan") || normalized.contains("jordon"),
        "text was: {text}"
    );
}
