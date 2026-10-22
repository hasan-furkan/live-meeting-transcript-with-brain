//! One lecture session: capture → VAD-gated STT → transcript, triggers and analyses → summary.
//!
//! The STT work runs on its own thread (whisper blocks); analyses and the summary run as
//! tasks on the caller's tokio runtime so transcription never waits for the network.

use crate::assist::{
    Analysis, MAX_TOKENS_ASSIST, MAX_TOKENS_CLASSIFY, assist_prompt, classify_prompt,
    parse_analysis, parse_classify,
};
use crate::capture::{AudioSource, CaptureError};
use crate::llm::{LlmClient, LlmError, LlmImage};
use crate::screenshot::capture as capture_screenshot;
use crate::settings::{Region, ScreenshotMode};
use crate::stt::{
    Backend, SegEvent, Segmenter, Transcriber, Vad, initial_prompt, is_hallucination,
};
use crate::summarize::summarize;
use crate::triggers::{HitKind, Matcher, RateLimiter, looks_like_question};
use crate::vault::{SessionFiles, TranscriptWriter, VaultError, write_summary};
use chrono::{Local, NaiveDate, NaiveTime};
use serde::Serialize;
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use ts_rs::TS;

/// How often the STT thread wakes to do timed work when no audio arrives.
const TICK: Duration = Duration::from_millis(250);
const LEVEL_INTERVAL: Duration = Duration::from_secs(1);
/// Below this RMS a second counts as silence.
const QUIET_RMS: f32 = 0.001;
const QUESTION_COOLDOWN: Duration = Duration::from_secs(30);
const RECENT_WORDS: usize = 350;
const CLASSIFY_WORDS: usize = 150;

#[derive(Serialize, TS, Clone, Debug)]
#[serde(tag = "type")]
#[ts(export)]
pub enum SessionEvent {
    Partial {
        text: String,
    },
    Line {
        time: String,
        text: String,
    },
    Hit {
        kind: HitKind,
        term: String,
        context: String,
    },
    AnalysisStarted {
        id: u64,
        reason: String,
    },
    Analysis {
        id: u64,
        time: String,
        analysis: Analysis,
    },
    AnalysisFailed {
        id: u64,
        error: String,
    },
    QuestionDetected {
        text: String,
    },
    Level {
        rms: f32,
    },
    NoAudio,
    Backend {
        backend: Backend,
    },
    SummaryWritten {
        path: String,
    },
    SummaryFailed {
        error: String,
    },
    Error {
        message: String,
    },
}

pub struct SessionConfig {
    pub course: String,
    pub files: SessionFiles,
    pub date: NaiveDate,
    pub names: Vec<String>,
    pub keywords: Vec<String>,
    pub general_questions: bool,
    pub name_bias: bool,
    pub screenshot: (ScreenshotMode, Option<String>, Option<Region>),
    /// Silence needed before `NoAudio` is reported; production uses 10 s.
    pub no_audio_after: Duration,
}

pub type Sink = Arc<dyn Fn(SessionEvent) + Send + Sync>;

#[derive(Debug)]
pub enum SessionError {
    Transcript(VaultError),
    Source(CaptureError),
    Thread,
    ReadTranscript,
    EmptyTranscript,
    Summary(VaultError),
    Llm(LlmError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transcript(e) => write!(f, "could not create transcript: {e}"),
            Self::Source(e) => write!(f, "{e}"),
            Self::Thread => write!(f, "could not start the session thread"),
            Self::ReadTranscript => write!(f, "could not read the transcript"),
            Self::EmptyTranscript => write!(f, "transcript is empty"),
            Self::Summary(e) => write!(f, "could not write summary: {e}"),
            Self::Llm(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SessionError {}

enum Command {
    AskNow,
    Finish,
}

pub struct Session {
    commands: Sender<Command>,
    join: JoinHandle<Vec<String>>,
    source: Box<dyn AudioSource>,
    files: SessionFiles,
    course: String,
    date: NaiveDate,
    llm: Option<Arc<LlmClient>>,
    rt: tokio::runtime::Handle,
    sink: Sink,
}

impl Session {
    pub fn start(
        cfg: SessionConfig,
        mut source: Box<dyn AudioSource>,
        stt: Box<dyn Transcriber>,
        vad: Box<dyn Vad>,
        llm: Option<Arc<LlmClient>>,
        rt: tokio::runtime::Handle,
        sink: Sink,
    ) -> Result<Session, SessionError> {
        let SessionConfig {
            course,
            files,
            date,
            names,
            keywords,
            general_questions,
            name_bias,
            screenshot,
            no_audio_after,
        } = cfg;

        let (audio_tx, audio_rx) = mpsc::channel();
        let (commands, command_rx) = mpsc::channel();
        source.start(audio_tx).map_err(SessionError::Source)?;
        let writer = match TranscriptWriter::create(&files.transcript, &course, date) {
            Ok(writer) => writer,
            Err(error) => {
                source.stop();
                return Err(SessionError::Transcript(error));
            }
        };

        let backend = stt.backend();
        sink(SessionEvent::Backend { backend });

        let worker = Worker {
            audio: audio_rx,
            commands: command_rx,
            sink: Arc::clone(&sink),
            segmenter: Segmenter::new(),
            vad,
            stt,
            matcher: Matcher::new(&names, &keywords),
            writer,
            prompt: initial_prompt(&names, name_bias),
            course: course.clone(),
            general_questions,
            screenshot,
            no_audio_after,
            question_rate: RateLimiter::new(QUESTION_COOLDOWN),
            pending: VecDeque::new(),
            append_failing: false,
            lines: Vec::new(),
            latest_partial: None,
            ids: Arc::new(AtomicU64::new(1)),
            llm: llm.clone(),
            rt: rt.clone(),
            level_sum: 0.0,
            level_count: 0,
            next_level: Instant::now() + LEVEL_INTERVAL,
            quiet_since: None,
            no_audio_sent: false,
        };
        let join = match std::thread::Builder::new()
            .name("lma-session".to_string())
            .spawn(move || worker.run())
        {
            Ok(join) => join,
            Err(_) => {
                source.stop();
                return Err(SessionError::Thread);
            }
        };

        Ok(Session {
            commands,
            join,
            source,
            files,
            course,
            date,
            llm,
            rt,
            sink,
        })
    }

    /// Non-blocking: the request is queued for the STT thread, which snapshots the context.
    pub fn ask_now(&self) {
        let _ = self.commands.send(Command::AskNow);
    }

    /// Stops capture and the STT thread, then summarizes on the runtime; the result
    /// arrives later as `SummaryWritten` or `SummaryFailed`.
    pub fn stop(self) -> Result<(), SessionError> {
        let Session {
            commands,
            join,
            mut source,
            files,
            course,
            date,
            llm,
            rt,
            sink,
        } = self;
        source.stop();
        let _ = commands.send(Command::Finish);
        drop(commands);
        let lines = join.join().map_err(|_| SessionError::Thread)?;
        let transcript = lines.join("\n");

        rt.spawn(async move {
            let Some(llm) = llm else {
                sink(SessionEvent::SummaryFailed {
                    error: "API key missing".to_string(),
                });
                return;
            };
            if transcript.trim().is_empty() {
                sink(SessionEvent::SummaryFailed {
                    error: "Transcript is empty".to_string(),
                });
                return;
            }
            let body = match summarize(&llm, &course, &transcript).await {
                Ok(body) => body,
                Err(error) => {
                    sink(SessionEvent::SummaryFailed {
                        error: error.to_string(),
                    });
                    return;
                }
            };
            match write_summary(&files, &course, date, &body) {
                Ok(()) => sink(SessionEvent::SummaryWritten {
                    path: files.summary.display().to_string(),
                }),
                Err(error) => sink(SessionEvent::SummaryFailed {
                    error: error.to_string(),
                }),
            }
        });
        Ok(())
    }

    /// Re-summarizes an existing transcript, e.g. after the summary call failed.
    pub async fn retry_summary(
        files: &SessionFiles,
        course: &str,
        date: NaiveDate,
        llm: &LlmClient,
    ) -> Result<(), SessionError> {
        let text =
            std::fs::read_to_string(&files.transcript).map_err(|_| SessionError::ReadTranscript)?;
        let transcript = transcript_text(&text);
        if transcript.trim().is_empty() {
            return Err(SessionError::EmptyTranscript);
        }
        let body = summarize(llm, course, &transcript)
            .await
            .map_err(SessionError::Llm)?;
        write_summary(files, course, date, &body).map_err(SessionError::Summary)
    }
}

/// Transcript body after the closing frontmatter line, without the `**[HH:MM:SS]** ` stamps.
fn transcript_text(text: &str) -> String {
    let mut lines = text.lines();
    if lines.next() == Some("---") {
        for line in lines.by_ref() {
            if line == "---" {
                break;
            }
        }
    } else {
        lines = text.lines();
    }
    lines
        .map(strip_stamp)
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_stamp(line: &str) -> &str {
    let Some(rest) = line.strip_prefix("**[") else {
        return line;
    };
    rest.split_once("]** ").map_or(line, |(_, text)| text)
}

struct Worker {
    audio: Receiver<Vec<f32>>,
    commands: Receiver<Command>,
    sink: Sink,
    segmenter: Segmenter,
    vad: Box<dyn Vad>,
    stt: Box<dyn Transcriber>,
    matcher: Matcher,
    writer: TranscriptWriter,
    prompt: Option<String>,
    course: String,
    general_questions: bool,
    screenshot: (ScreenshotMode, Option<String>, Option<Region>),
    no_audio_after: Duration,
    question_rate: RateLimiter,
    /// Lines that could not be written yet, oldest first.
    pending: VecDeque<(NaiveTime, String)>,
    append_failing: bool,
    /// Every final line, for the summary.
    lines: Vec<String>,
    latest_partial: Option<String>,
    ids: Arc<AtomicU64>,
    llm: Option<Arc<LlmClient>>,
    rt: tokio::runtime::Handle,
    level_sum: f32,
    level_count: usize,
    next_level: Instant,
    quiet_since: Option<Instant>,
    no_audio_sent: bool,
}

impl Worker {
    fn run(mut self) -> Vec<String> {
        let mut audio_done = false;
        loop {
            if audio_done {
                match self.commands.recv_timeout(TICK) {
                    Ok(Command::AskNow) => self.analyze("manual"),
                    Ok(Command::Finish) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match self.audio.recv_timeout(TICK) {
                    Ok(chunk) => self.ingest(&chunk),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => audio_done = true,
                }
                if self.handle_commands() {
                    break;
                }
            }
            self.tick_level();
        }

        if let Some(segment) = self.segmenter.flush() {
            self.final_line(&segment);
        }
        self.retry_pending();
        self.lines
    }

    fn handle_commands(&mut self) -> bool {
        loop {
            match self.commands.try_recv() {
                Ok(Command::AskNow) => self.analyze("manual"),
                Ok(Command::Finish) => return true,
                Err(TryRecvError::Empty) => return false,
                Err(TryRecvError::Disconnected) => return true,
            }
        }
    }

    fn ingest(&mut self, chunk: &[f32]) {
        self.level_sum += chunk.iter().map(|sample| sample * sample).sum::<f32>();
        self.level_count += chunk.len();
        for event in self.segmenter.push(chunk, self.vad.as_mut()) {
            match event {
                SegEvent::Fast(samples) => self.fast_pass(&samples),
                SegEvent::Final(samples) => self.final_line(&samples),
            }
        }
    }

    /// The VAD gate lives in the segmenter: both passes only see speech segments.
    fn transcribe(&mut self, samples: &[f32]) -> Option<String> {
        let prompt = self.prompt.as_deref();
        let text = self.stt.transcribe(samples, prompt).ok()?;
        if is_hallucination(&text, prompt) {
            return None;
        }
        Some(text)
    }

    fn fast_pass(&mut self, samples: &[f32]) {
        let Some(text) = self.transcribe(samples) else {
            return;
        };
        (self.sink)(SessionEvent::Partial { text: text.clone() });
        self.latest_partial = Some(text.clone());
        self.scan(&text);
    }

    fn final_line(&mut self, samples: &[f32]) {
        let Some(text) = self.transcribe(samples) else {
            return;
        };
        let time = Local::now().time();
        self.append(time, &text);
        self.lines.push(text.clone());
        (self.sink)(SessionEvent::Line {
            time: time.format("%H:%M:%S").to_string(),
            text: text.clone(),
        });
        self.scan(&text);
        self.check_question(&text);
    }

    fn scan(&mut self, text: &str) {
        for hit in self.matcher.scan(text, Instant::now()) {
            let is_name = hit.kind == HitKind::Name;
            (self.sink)(SessionEvent::Hit {
                kind: hit.kind,
                term: hit.term,
                context: hit.context,
            });
            if is_name {
                self.analyze("name");
            }
        }
    }

    /// Runs the trigger decision on a final line; the classification call is off-thread.
    fn check_question(&mut self, line: &str) {
        if !self.general_questions || !looks_like_question(line) {
            return;
        }
        let Some(llm) = self.llm.clone() else {
            return;
        };
        if !self.question_rate.allow(Instant::now()) {
            return;
        }

        let recent = self.recent_text();
        let classify_recent = last_words(&recent, CLASSIFY_WORDS);
        let analysis_recent = last_words(&recent, RECENT_WORDS);
        let course = self.course.clone();
        let screenshot = self.screenshot.clone();
        let sink = Arc::clone(&self.sink);
        let ids = Arc::clone(&self.ids);
        let text = line.to_string();
        self.rt.spawn(async move {
            let (system, user) = classify_prompt(&course, &classify_recent);
            let Ok(reply) = llm
                .complete(&system, &user, None, MAX_TOKENS_CLASSIFY)
                .await
            else {
                return;
            };
            if !parse_classify(&reply) {
                return;
            }
            sink(SessionEvent::QuestionDetected { text });
            let id = ids.fetch_add(1, Ordering::Relaxed);
            sink(SessionEvent::AnalysisStarted {
                id,
                reason: "question".to_string(),
            });
            run_analysis(llm, id, course, analysis_recent, screenshot, sink).await;
        });
    }

    fn analyze(&mut self, reason: &str) {
        let id = self.ids.fetch_add(1, Ordering::Relaxed);
        (self.sink)(SessionEvent::AnalysisStarted {
            id,
            reason: reason.to_string(),
        });
        let Some(llm) = self.llm.clone() else {
            (self.sink)(SessionEvent::AnalysisFailed {
                id,
                error: "API key missing".to_string(),
            });
            return;
        };
        let recent = last_words(&self.recent_text(), RECENT_WORDS);
        let course = self.course.clone();
        let screenshot = self.screenshot.clone();
        let sink = Arc::clone(&self.sink);
        self.rt.spawn(async move {
            run_analysis(llm, id, course, recent, screenshot, sink).await;
        });
    }

    /// All final lines plus the latest partial, as one space-separated string.
    fn recent_text(&self) -> String {
        let mut text = self.lines.join(" ");
        if let Some(partial) = &self.latest_partial {
            text.push(' ');
            text.push_str(partial);
        }
        text
    }

    fn append(&mut self, time: NaiveTime, text: &str) {
        while let Some((pending_time, pending_text)) =
            self.pending.front().map(|(t, s)| (*t, s.clone()))
        {
            if self.writer.append(pending_time, &pending_text).is_err() {
                self.report_append_failure();
                return;
            }
            self.pending.pop_front();
        }
        match self.writer.append(time, text) {
            Ok(()) => self.append_failing = false,
            Err(_) => {
                self.pending.push_back((time, text.to_string()));
                self.report_append_failure();
            }
        }
    }

    fn retry_pending(&mut self) {
        while let Some((time, text)) = self.pending.front().map(|(t, s)| (*t, s.clone())) {
            if self.writer.append(time, &text).is_err() {
                self.report_append_failure();
                return;
            }
            self.pending.pop_front();
        }
        self.append_failing = false;
    }

    fn report_append_failure(&mut self) {
        if !self.append_failing {
            self.append_failing = true;
            (self.sink)(SessionEvent::Error {
                message: "Could not write transcript; retrying".to_string(),
            });
        }
    }

    fn tick_level(&mut self) {
        let now = Instant::now();
        if now < self.next_level {
            return;
        }
        self.next_level = now + LEVEL_INTERVAL;
        let count = self.level_count;
        let sum = std::mem::take(&mut self.level_sum);
        self.level_count = 0;
        let rms = if count == 0 {
            0.0
        } else {
            (sum / count as f32).sqrt()
        };
        (self.sink)(SessionEvent::Level { rms });

        if rms < QUIET_RMS {
            let since = *self.quiet_since.get_or_insert(now);
            if !self.no_audio_sent && now.duration_since(since) >= self.no_audio_after {
                self.no_audio_sent = true;
                (self.sink)(SessionEvent::NoAudio);
            }
        } else {
            self.quiet_since = None;
            self.no_audio_sent = false;
        }
    }
}

async fn run_analysis(
    llm: Arc<LlmClient>,
    id: u64,
    course: String,
    recent: String,
    screenshot: (ScreenshotMode, Option<String>, Option<Region>),
    sink: Sink,
) {
    let image = capture_image(screenshot).await;
    let (system, user) = assist_prompt(&course, &recent, image.is_some());
    let event = match llm
        .complete(&system, &user, image.as_ref(), MAX_TOKENS_ASSIST)
        .await
    {
        Ok(reply) => SessionEvent::Analysis {
            id,
            time: clock(),
            analysis: parse_analysis(&reply),
        },
        Err(error) => SessionEvent::AnalysisFailed {
            id,
            error: error.to_string(),
        },
    };
    sink(event);
}

/// Screenshots are taken on a blocking thread; a failure just means text-only analysis.
async fn capture_image(
    (mode, monitor, region): (ScreenshotMode, Option<String>, Option<Region>),
) -> Option<LlmImage> {
    if mode == ScreenshotMode::Off {
        return None;
    }
    tokio::task::spawn_blocking(move || capture_screenshot(mode, monitor.as_deref(), region))
        .await
        .ok()
        .and_then(Result::ok)
        .flatten()
}

fn clock() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

fn last_words(text: &str, count: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    words[words.len().saturating_sub(count)..].join(" ")
}
