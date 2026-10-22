#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::NaiveDate;
use lma_core::capture::{AudioSource, CaptureError, WavSource};
use lma_core::llm::LlmClient;
use lma_core::session::{Session, SessionConfig, SessionEvent, Sink};
use lma_core::settings::ScreenshotMode;
use lma_core::stt::{
    Backend, EarshotVad, SttError, Transcriber, Vad, WhisperTranscriber, download_model,
    model_file_name,
};
use lma_core::triggers::HitKind;
use lma_core::vault::allocate_session;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;

const WAIT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------- audio input

/// The test drives the audio timeline itself: `start` parks the session's sender.
#[derive(Clone, Default)]
struct Input(Arc<Mutex<Option<Sender<Vec<f32>>>>>);

impl AudioSource for Input {
    fn start(&mut self, tx: Sender<Vec<f32>>) -> Result<(), CaptureError> {
        *self.0.lock().unwrap() = Some(tx);
        Ok(())
    }

    fn stop(&mut self) {
        self.0.lock().unwrap().take();
    }
}

impl Input {
    fn push(&self, samples: &[f32]) {
        let tx = self.0.lock().unwrap().clone();
        if let Some(tx) = tx {
            for chunk in samples.chunks(1_600) {
                let _ = tx.send(chunk.to_vec());
            }
        }
    }

    fn speech(&self, ms: usize) {
        self.push(&tone(ms * 16));
    }

    fn silence(&self, ms: usize) {
        self.push(&vec![0.0; ms * 16]);
    }

    /// One 100 ms chunk at a time, so wall-clock timers see the silence.
    fn silence_realtime(&self, ms: usize) {
        let samples = vec![0.0f32; ms * 16];
        for chunk in samples.chunks(1_600) {
            self.push(chunk);
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn tone(samples: usize) -> Vec<f32> {
    (0..samples)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16_000.0).sin())
        .collect()
}

/// Speech when any sample is louder than 0.1; the tone is 0.5, silence 0.0.
struct ThresholdVad;

impl Vad for ThresholdVad {
    fn is_speech(&mut self, frame: &[f32]) -> bool {
        frame.iter().any(|s| s.abs() > 0.1)
    }
}

/// Fast windows are at least 2 s of audio (>= 32 000 samples); the final segments in
/// these tests are ~1.3 s, so the length tells the two passes apart.
struct FakeTranscriber {
    fast: String,
    finals: std::collections::VecDeque<String>,
}

impl FakeTranscriber {
    fn new(fast: &str, finals: &[&str]) -> Self {
        Self {
            fast: fast.to_string(),
            finals: finals.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl Transcriber for FakeTranscriber {
    fn transcribe(&mut self, pcm16k: &[f32], _prompt: Option<&str>) -> Result<String, SttError> {
        if pcm16k.len() >= 30_000 {
            return Ok(self.fast.clone());
        }
        Ok(self.finals.pop_front().unwrap_or_default())
    }

    fn backend(&self) -> Backend {
        Backend::Cpu
    }
}

// ------------------------------------------------------------------- helpers

fn config(notes: &Path) -> SessionConfig {
    let date = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
    let files = allocate_session(notes, "Algorithms 101", date).unwrap();
    SessionConfig {
        course: "Algorithms 101".to_string(),
        files,
        date,
        names: vec!["Jordan".to_string()],
        keywords: vec!["exam".to_string()],
        general_questions: false,
        name_bias: true,
        screenshot: (ScreenshotMode::Off, None, None),
        no_audio_after: Duration::from_secs(60),
    }
}

fn collector() -> (Sink, Arc<Mutex<Vec<SessionEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink_events = Arc::clone(&events);
    let sink: Sink = Arc::new(move |event| sink_events.lock().unwrap().push(event));
    (sink, events)
}

fn snapshot(events: &Arc<Mutex<Vec<SessionEvent>>>) -> Vec<SessionEvent> {
    events.lock().unwrap().clone()
}

fn wait_for(
    events: &Arc<Mutex<Vec<SessionEvent>>>,
    timeout: Duration,
    done: impl Fn(&[SessionEvent]) -> bool,
) -> Vec<SessionEvent> {
    let deadline = Instant::now() + timeout;
    loop {
        let now = snapshot(events);
        if done(&now) {
            return now;
        }
        assert!(Instant::now() < deadline, "timed out; events: {now:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn position(events: &[SessionEvent], wanted: impl Fn(&SessionEvent) -> bool) -> Option<usize> {
    events.iter().position(wanted)
}

fn set_readonly(path: &Path, readonly: bool) {
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    #[cfg(windows)]
    permissions.set_readonly(readonly);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(if readonly { 0o444 } else { 0o644 });
    }
    std::fs::set_permissions(path, permissions).unwrap();
}

// ---------------------------------------------------------------- mock server

struct MockServer {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl MockServer {
    /// Replies are used in order; the last one repeats.
    async fn start(responses: Vec<(u16, String)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        tokio::spawn(async move {
            let mut served = 0_usize;
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let Some(body) = read_request(&mut stream).await else {
                    continue;
                };
                if let Ok(mut log) = recorded.lock() {
                    log.push(body);
                }
                let (status, body) = responses
                    .get(served)
                    .or_else(|| responses.last())
                    .cloned()
                    .unwrap_or((500, String::from("{}")));
                served += 1;
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });
        Self { url, requests }
    }

    fn bodies(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

async fn read_request(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut content_length = 0_usize;
    for line in head.split("\r\n").skip(1) {
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);
    Some(String::from_utf8_lossy(&body).into_owned())
}

fn text_reply(text: &str) -> String {
    json!({ "content": [{ "type": "text", "text": text }] }).to_string()
}

const ASSIST_JSON: &str = r#"{"question":"What is merge sort?","suggested_answer":"A divide and conquer sort.","confidence":"high"}"#;

fn start_fake(
    rt: &Runtime,
    cfg: SessionConfig,
    stt: FakeTranscriber,
    llm: Option<Arc<LlmClient>>,
    input: &Input,
) -> (Session, Arc<Mutex<Vec<SessionEvent>>>) {
    let (sink, events) = collector();
    let session = Session::start(
        cfg,
        Box::new(input.clone()),
        Box::new(stt),
        Box::new(ThresholdVad),
        llm,
        rt.handle().clone(),
        sink,
    )
    .unwrap();
    (session, events)
}

// -------------------------------------------------------------------- tests

#[test]
fn name_in_fast_pass_emits_hit_then_analysis() {
    let rt = Runtime::new().unwrap();
    let server = rt.block_on(MockServer::start(vec![(200, text_reply(ASSIST_JSON))]));
    let llm = Arc::new(LlmClient::new(&server.url, "test-key".into(), "test-model"));
    let dir = tempfile::tempdir().unwrap();
    let input = Input::default();
    let (session, events) = start_fake(
        &rt,
        config(dir.path()),
        FakeTranscriber::new("Jordan, what is merge sort?", &[]),
        Some(llm),
        &input,
    );

    input.speech(2_000); // the first 2 s fast window

    let seen = wait_for(&events, WAIT, |e| {
        e.iter().any(|x| matches!(x, SessionEvent::Analysis { .. }))
    });

    let hit = position(
        &seen,
        |e| matches!(e, SessionEvent::Hit { kind: HitKind::Name, term, .. } if term == "Jordan"),
    )
    .expect("name hit");
    let started = position(
        &seen,
        |e| matches!(e, SessionEvent::AnalysisStarted { reason, .. } if reason == "name"),
    )
    .expect("analysis started");
    let analysed = position(&seen, |e| matches!(e, SessionEvent::Analysis { .. })).unwrap();
    assert!(hit < started && started < analysed, "{seen:?}");

    let (id, analysis) = seen
        .iter()
        .find_map(|e| match e {
            SessionEvent::Analysis { id, analysis, .. } => Some((*id, analysis.clone())),
            _ => None,
        })
        .unwrap();
    assert_eq!(analysis.question, "What is merge sort?");
    assert_eq!(analysis.suggested_answer, "A divide and conquer sort.");
    assert_eq!(
        analysis.confidence,
        Some(lma_core::assist::Confidence::High)
    );
    assert!(
        seen.iter().any(
            |e| matches!(e, SessionEvent::AnalysisStarted { id: started_id, .. } if *started_id == id)
        ),
        "{seen:?}"
    );
    assert!(
        seen.iter().any(|e| matches!(
            e,
            SessionEvent::Backend {
                backend: Backend::Cpu
            }
        )),
        "{seen:?}"
    );

    let bodies = server.bodies();
    assert_eq!(bodies.len(), 1);
    let request: Value = serde_json::from_str(&bodies[0]).unwrap();
    let content = request["messages"][0]["content"].as_array().unwrap();
    assert_eq!(content.len(), 1, "screenshot is off: {request}");
    let prompt = content[0]["text"].as_str().unwrap();
    assert!(prompt.contains("Course: Algorithms 101"), "{prompt}");
    assert!(prompt.contains("Jordan, what is merge sort?"), "{prompt}");

    session.stop().unwrap();
}

#[test]
fn silence_wav_produces_no_lines_or_hits() {
    let rt = Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let input = Input::default();
    let (session, events) = start_fake(
        &rt,
        config(dir.path()),
        FakeTranscriber::new("Jordan is here", &["Jordan is here"]),
        None,
        &input,
    );

    input.silence(1_500);
    // A level tick proves the audio was consumed and silence was measured.
    wait_for(&events, WAIT, |e| {
        e.iter().any(|x| matches!(x, SessionEvent::Level { .. }))
    });
    let seen = snapshot(&events);
    assert!(
        !seen.iter().any(|e| matches!(
            e,
            SessionEvent::Partial { .. } | SessionEvent::Line { .. } | SessionEvent::Hit { .. }
        )),
        "{seen:?}"
    );

    session.stop().unwrap();
}

#[test]
fn transcript_file_gets_final_lines() {
    let rt = Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path());
    let transcript = cfg.files.transcript.clone();
    let input = Input::default();
    let (session, events) = start_fake(
        &rt,
        cfg,
        FakeTranscriber::new("", &["Today we cover merge sort"]),
        None,
        &input,
    );

    input.speech(500);
    input.silence(1_000);

    let seen = wait_for(&events, WAIT, |e| {
        e.iter().any(|x| matches!(x, SessionEvent::Line { .. }))
    });
    let (time, text) = seen
        .iter()
        .find_map(|e| match e {
            SessionEvent::Line { time, text } => Some((time.clone(), text.clone())),
            _ => None,
        })
        .unwrap();
    assert_eq!(text, "Today we cover merge sort");
    assert_eq!(time.len(), 8, "{time}");
    assert_eq!(&time[2..3], ":");
    assert_eq!(&time[5..6], ":");

    session.stop().unwrap();
    let file = std::fs::read_to_string(&transcript).unwrap();
    assert!(
        file.contains(&format!("**[{time}]** Today we cover merge sort")),
        "{file}"
    );
    assert_eq!(file.lines().filter(|l| l.starts_with("**[")).count(), 1);
}

#[test]
fn append_failure_is_buffered_and_retried() {
    let rt = Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path());
    let transcript = cfg.files.transcript.clone();
    let input = Input::default();
    let (session, events) = start_fake(
        &rt,
        cfg,
        FakeTranscriber::new("", &["first line", "second line", "third line"]),
        None,
        &input,
    );

    input.speech(500);
    input.silence(1_000);
    wait_for(&events, WAIT, |e| {
        e.iter()
            .any(|x| matches!(x, SessionEvent::Line { text, .. } if text == "first line"))
    });

    set_readonly(&transcript, true);
    input.speech(500);
    input.silence(1_000);
    let seen = wait_for(&events, WAIT, |e| {
        e.iter()
            .any(|x| matches!(x, SessionEvent::Line { text, .. } if text == "second line"))
            && e.iter().any(|x| matches!(x, SessionEvent::Error { .. }))
    });
    assert_eq!(
        seen.iter()
            .filter(|e| matches!(e, SessionEvent::Error { .. }))
            .count(),
        1,
        "one error per failure streak: {seen:?}"
    );

    set_readonly(&transcript, false);
    input.speech(500);
    input.silence(1_000);
    wait_for(&events, WAIT, |e| {
        e.iter()
            .any(|x| matches!(x, SessionEvent::Line { text, .. } if text == "third line"))
    });

    session.stop().unwrap();
    let file = std::fs::read_to_string(&transcript).unwrap();
    let first = file.find("first line").unwrap();
    let second = file.find("second line").unwrap();
    let third = file.find("third line").unwrap();
    assert!(first < second && second < third, "{file}");
    assert_eq!(file.lines().filter(|l| l.starts_with("**[")).count(), 3);
}

#[test]
fn no_audio_event_after_10s_of_silence() {
    let rt = Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path());
    cfg.no_audio_after = Duration::from_secs(2);
    let input = Input::default();
    let (session, events) = start_fake(&rt, cfg, FakeTranscriber::new("", &[]), None, &input);

    input.silence_realtime(2_500);

    let seen = wait_for(&events, WAIT, |e| {
        e.iter().any(|x| matches!(x, SessionEvent::NoAudio))
    });
    assert_eq!(
        seen.iter()
            .filter(|e| matches!(e, SessionEvent::NoAudio))
            .count(),
        1,
        "NoAudio fires once while quiet: {seen:?}"
    );

    session.stop().unwrap();
}

#[test]
fn missing_llm_reports_api_key_missing() {
    let rt = Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let input = Input::default();
    let (session, events) = start_fake(
        &rt,
        config(dir.path()),
        FakeTranscriber::new("Jordan, are you there?", &[]),
        None,
        &input,
    );

    input.speech(2_000);

    let seen = wait_for(&events, WAIT, |e| {
        e.iter()
            .any(|x| matches!(x, SessionEvent::AnalysisFailed { .. }))
    });
    let started = position(
        &seen,
        |e| matches!(e, SessionEvent::AnalysisStarted { reason, .. } if reason == "name"),
    )
    .expect("analysis started");
    let failed = position(
        &seen,
        |e| matches!(e, SessionEvent::AnalysisFailed { error, .. } if error == "API key missing"),
    )
    .expect("analysis failed");
    assert!(started < failed, "{seen:?}");
    let failed_id = seen
        .iter()
        .find_map(|e| match e {
            SessionEvent::AnalysisFailed { id, .. } => Some(*id),
            _ => None,
        })
        .unwrap();
    assert!(
        seen.iter()
            .any(|e| matches!(e, SessionEvent::AnalysisStarted { id, .. } if *id == failed_id)),
        "{seen:?}"
    );

    session.stop().unwrap();
    wait_for(&events, WAIT, |e| {
        e.iter().any(
            |x| matches!(x, SessionEvent::SummaryFailed { error } if error == "API key missing"),
        )
    });
}

// ------------------------------------------------------------ real tiny model

fn model_dir() -> PathBuf {
    std::env::var_os("LMA_TEST_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap()).join("lma-test-models")
        })
}

/// The fixture plus 2 s of silence so the VAD closes a final segment.
fn wav_with_trailing_silence(path: &Path) {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/jordan.wav");
    let mut reader = hound::WavReader::open(fixture).unwrap();
    let spec = reader.spec();
    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for sample in samples {
        writer.write_sample(sample).unwrap();
    }
    for _ in 0..32_000 {
        writer.write_sample(0_i16).unwrap();
    }
    writer.finalize().unwrap();
}

#[test]
#[ignore = "needs ggml-tiny.bin and a GPU build (`--features cuda -- --ignored`)"]
fn end_to_end_tiny_model() {
    let rt = Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let model = model_dir();
    let model_path = model.join(model_file_name("tiny"));
    if !model_path.is_file() {
        println!("downloading ggml-tiny.bin to {}", model.display());
        rt.block_on(download_model("tiny", &model, |_, _| {}))
            .unwrap();
    }

    let wav = dir.path().join("jordan-silence.wav");
    wav_with_trailing_silence(&wav);
    let stt = WhisperTranscriber::load(&model_path, "en").unwrap();

    let cfg = config(dir.path());
    let (sink, events) = collector();
    let session = Session::start(
        cfg,
        Box::new(WavSource::new(wav, false)),
        Box::new(stt),
        Box::new(EarshotVad::new()),
        None,
        rt.handle().clone(),
        sink,
    )
    .unwrap();

    let seen = wait_for(&events, Duration::from_secs(120), |e| {
        e.iter().any(|x| {
            matches!(
                x,
                SessionEvent::Hit {
                    kind: HitKind::Name,
                    ..
                }
            )
        }) && e.iter().any(|x| matches!(x, SessionEvent::Line { .. }))
    });
    assert!(
        seen.iter()
            .any(|e| matches!(e, SessionEvent::Hit { term, .. } if term == "Jordan")),
        "{seen:?}"
    );

    session.stop().unwrap();
    wait_for(&events, WAIT, |e| {
        e.iter().any(
            |x| matches!(x, SessionEvent::SummaryFailed { error } if error == "API key missing"),
        )
    });

    let transcript = std::fs::read_to_string(
        dir.path()
            .join("Algorithms 101")
            .join("2026-10-22-transcript.md"),
    )
    .unwrap();
    assert!(transcript.lines().filter(|l| l.starts_with("**[")).count() >= 1);
}
