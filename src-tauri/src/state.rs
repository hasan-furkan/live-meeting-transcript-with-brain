//! Pure app state shared by the commands: no Tauri runtime, so it is unit-testable.

use chrono::NaiveDate;
use lma_core::session::{Session, SessionEvent};
use lma_core::settings::Settings;
use lma_core::triggers::HitKind;
use lma_core::vault::SessionFiles;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;
use ts_rs::TS;

pub const NOTES_MISSING: &str = "Set the notes folder in Settings first";
pub const MODEL_MISSING: &str = "Download the Whisper model first";
pub const NO_SESSION: &str = "No session is running";
pub const SESSION_RUNNING: &str = "A session is already running";
pub const STOP_FIRST: &str = "Stop the running session first";

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    pub models_dir: PathBuf,
    pub gate: Mutex<SessionGate>,
    pub session: Mutex<Option<Session>>,
    pub last: Mutex<Option<(SessionFiles, String, NaiveDate)>>,
    pub summary: Arc<SummaryTracker>,
    pub runtime: tokio::runtime::Handle,
}

/// Whether a summary requested by `Session::stop` is still in flight, so Quit can wait for it.
pub struct SummaryTracker {
    pending: Mutex<bool>,
    done: Condvar,
}

impl SummaryTracker {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(false),
            done: Condvar::new(),
        }
    }

    /// Called before `Session::stop`, so the summary's completion cannot race past the wait.
    pub fn begin(&self) {
        *lock(&self.pending) = true;
    }

    /// Called by the sink on `SummaryWritten` / `SummaryFailed`.
    pub fn complete(&self) {
        *lock(&self.pending) = false;
        self.done.notify_all();
    }

    /// Blocks until no summary is pending or `timeout` elapses; true when none is pending.
    pub fn wait(&self, timeout: Duration) -> bool {
        let pending = lock(&self.pending);
        let (pending, _) = self
            .done
            .wait_timeout_while(pending, timeout, |pending| *pending)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        !*pending
    }
}

impl Default for SummaryTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Poisoned locks still hold valid data; a panicking reader must not brick the app.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Running,
}

pub struct SessionGate {
    phase: Phase,
}

impl SessionGate {
    pub fn new() -> Self {
        Self { phase: Phase::Idle }
    }

    pub fn try_begin(&mut self) -> Result<(), String> {
        match self.phase {
            Phase::Running => Err(SESSION_RUNNING.to_string()),
            Phase::Idle => {
                self.phase = Phase::Running;
                Ok(())
            }
        }
    }

    pub fn end(&mut self) -> Result<(), String> {
        match self.phase {
            Phase::Idle => Err(NO_SESSION.to_string()),
            Phase::Running => {
                self.phase = Phase::Idle;
                Ok(())
            }
        }
    }

    pub fn require_running(&self) -> Result<(), String> {
        match self.phase {
            Phase::Running => Ok(()),
            Phase::Idle => Err(NO_SESSION.to_string()),
        }
    }

    /// For actions that would race a live session, such as a summary retry.
    pub fn require_idle(&self) -> Result<(), String> {
        match self.phase {
            Phase::Idle => Ok(()),
            Phase::Running => Err(STOP_FIRST.to_string()),
        }
    }
}

impl Default for SessionGate {
    fn default() -> Self {
        Self::new()
    }
}

/// The notes folder, but only when the session could actually start.
pub fn check_start(settings: &Settings, model_installed: bool) -> Result<PathBuf, String> {
    let notes = settings
        .notes_dir
        .clone()
        .ok_or_else(|| NOTES_MISSING.to_string())?;
    if !model_installed {
        return Err(MODEL_MISSING.to_string());
    }
    Ok(notes)
}

#[derive(Serialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct ModelStatus {
    pub installed: bool,
    pub model: String,
}

#[derive(Serialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct ModelProgress {
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
}

/// Notification title and body for the events the user must not miss.
pub fn notification_for(event: &SessionEvent) -> Option<(String, String)> {
    match event {
        SessionEvent::Hit {
            kind,
            term,
            context,
        } => match kind {
            HitKind::Name => Some((
                "Your name was mentioned".to_string(),
                last_chars(context, 120),
            )),
            HitKind::Keyword => Some((format!("Keyword: {term}"), last_chars(context, 120))),
        },
        SessionEvent::Analysis { analysis, .. } => {
            let text = if analysis.question.is_empty() {
                analysis.raw.clone().unwrap_or_default()
            } else {
                format!("{} → {}", analysis.question, analysis.suggested_answer)
            };
            Some(("Question".to_string(), first_chars(&text, 200)))
        }
        SessionEvent::QuestionDetected { text } => {
            Some(("Question to the class".to_string(), first_chars(text, 120)))
        }
        SessionEvent::NoAudio => Some((
            "No audio detected".to_string(),
            "Check the output device".to_string(),
        )),
        SessionEvent::SummaryWritten { path } => {
            let name = std::path::Path::new(path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            Some(("Lecture notes saved".to_string(), name))
        }
        SessionEvent::SummaryFailed { error } => {
            Some(("Summary failed".to_string(), error.clone()))
        }
        _ => None,
    }
}

/// Question-to-the-class notices must not play a sound.
pub fn notification_is_silent(event: &SessionEvent) -> bool {
    matches!(event, SessionEvent::QuestionDetected { .. })
}

/// The most recent `max` chars, marked from the front when something was cut.
fn last_chars(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let tail: String = text.chars().skip(count - max).collect();
    format!("…{tail}")
}

/// The first `max` chars, including the marker when something was cut.
fn first_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let head: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{head}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use lma_core::assist::{Analysis, Confidence};
    use lma_core::session::ScreenshotStatus;

    fn notes_settings() -> Settings {
        Settings {
            notes_dir: Some(PathBuf::from("notes")),
            ..Settings::default()
        }
    }

    fn hit(kind: HitKind, term: &str, context: &str) -> SessionEvent {
        SessionEvent::Hit {
            kind,
            term: term.to_string(),
            context: context.to_string(),
        }
    }

    #[test]
    fn second_start_rejected() {
        let mut gate = SessionGate::new();
        assert_eq!(gate.try_begin(), Ok(()));
        assert_eq!(gate.try_begin(), Err(SESSION_RUNNING.to_string()));
    }

    #[test]
    fn stop_without_session_errors() {
        let mut gate = SessionGate::new();
        assert_eq!(gate.end(), Err(NO_SESSION.to_string()));
    }

    #[test]
    fn ask_now_without_session_errors() {
        let gate = SessionGate::new();
        assert_eq!(gate.require_running(), Err(NO_SESSION.to_string()));
    }

    #[test]
    fn retry_summary_is_rejected_while_a_session_runs() {
        let mut gate = SessionGate::new();
        assert_eq!(gate.require_idle(), Ok(()));
        assert_eq!(gate.try_begin(), Ok(()));
        assert_eq!(gate.require_idle(), Err(STOP_FIRST.to_string()));
    }

    #[test]
    fn start_requires_notes_dir_and_model() {
        assert_eq!(
            check_start(&Settings::default(), true),
            Err(NOTES_MISSING.to_string())
        );
        assert_eq!(
            check_start(&Settings::default(), false),
            Err(NOTES_MISSING.to_string())
        );
        assert_eq!(
            check_start(&notes_settings(), false),
            Err(MODEL_MISSING.to_string())
        );
        assert_eq!(
            check_start(&notes_settings(), true),
            Ok(PathBuf::from("notes"))
        );
    }

    #[test]
    fn name_hit_notification_keeps_last_120_chars() {
        let context = format!("{}name", "x".repeat(200));
        let (title, body) = notification_for(&hit(HitKind::Name, "Jordan", &context)).unwrap();
        assert_eq!(title, "Your name was mentioned");
        assert_eq!(body.chars().count(), 121);
        assert!(body.starts_with('…'));
        assert!(body.ends_with("name"));

        let short = "was said to Jordan just now";
        let (_, body) = notification_for(&hit(HitKind::Name, "Jordan", short)).unwrap();
        assert_eq!(body, short);
    }

    #[test]
    fn keyword_hit_notification_names_the_term() {
        let (title, body) =
            notification_for(&hit(HitKind::Keyword, "exam", "the exam is next")).unwrap();
        assert_eq!(title, "Keyword: exam");
        assert_eq!(body, "the exam is next");
    }

    #[test]
    fn analysis_notification_pairs_question_and_answer() {
        let event = SessionEvent::Analysis {
            id: 1,
            time: "10:00:00".to_string(),
            analysis: Analysis {
                question: "What is a hash map?".to_string(),
                suggested_answer: "A key-value store.".to_string(),
                confidence: Some(Confidence::High),
                explanation: None,
                raw: None,
            },
            screenshot: ScreenshotStatus::Off,
        };
        assert_eq!(
            notification_for(&event),
            Some((
                "Question".to_string(),
                "What is a hash map? → A key-value store.".to_string()
            ))
        );
    }

    #[test]
    fn analysis_notification_falls_back_to_raw_text() {
        let event = SessionEvent::Analysis {
            id: 2,
            time: "10:00:01".to_string(),
            analysis: Analysis {
                question: String::new(),
                suggested_answer: String::new(),
                confidence: None,
                explanation: None,
                raw: Some("not json at all".to_string()),
            },
            screenshot: ScreenshotStatus::Off,
        };
        assert_eq!(
            notification_for(&event),
            Some(("Question".to_string(), "not json at all".to_string()))
        );
    }

    #[test]
    fn analysis_notification_truncates_to_200_chars() {
        let event = SessionEvent::Analysis {
            id: 3,
            time: "10:00:02".to_string(),
            analysis: Analysis {
                question: "q".repeat(150),
                suggested_answer: "a".repeat(150),
                confidence: None,
                explanation: None,
                raw: None,
            },
            screenshot: ScreenshotStatus::Off,
        };
        let (_, body) = notification_for(&event).unwrap();
        assert_eq!(body.chars().count(), 200);
        assert!(body.ends_with('…'));
        assert!(body.starts_with(&format!("{} → ", "q".repeat(150))));
    }

    #[test]
    fn question_detected_notification_truncates_to_120_chars() {
        let event = SessionEvent::QuestionDetected {
            text: "ş".repeat(130),
        };
        let (title, body) = notification_for(&event).unwrap();
        assert_eq!(title, "Question to the class");
        assert_eq!(body.chars().count(), 120);
        assert!(body.ends_with('…'));
        assert!(body.starts_with(&"ş".repeat(119)));
    }

    #[test]
    fn question_detected_notification_is_silent() {
        assert!(notification_is_silent(&SessionEvent::QuestionDetected {
            text: "anyone?".to_string()
        }));
        assert!(!notification_is_silent(&SessionEvent::NoAudio));
        assert!(!notification_is_silent(&hit(
            HitKind::Name,
            "Jordan",
            "Jordan"
        )));
    }

    #[test]
    fn summary_tracker_is_pending_between_begin_and_complete() {
        let tracker = SummaryTracker::new();
        assert!(tracker.wait(Duration::from_millis(1)));

        tracker.begin();
        assert!(!tracker.wait(Duration::from_millis(10)));

        tracker.complete();
        assert!(tracker.wait(Duration::from_millis(10)));
    }

    #[test]
    fn summary_tracker_wait_wakes_on_complete() {
        let tracker = Arc::new(SummaryTracker::new());
        tracker.begin();
        let completer = Arc::clone(&tracker);
        std::thread::spawn(move || completer.complete());
        assert!(tracker.wait(Duration::from_secs(5)));
    }

    #[test]
    fn no_audio_notification() {
        assert_eq!(
            notification_for(&SessionEvent::NoAudio),
            Some((
                "No audio detected".to_string(),
                "Check the output device".to_string()
            ))
        );
    }

    #[test]
    fn summary_written_notification_shows_only_the_file_name() {
        assert_eq!(
            notification_for(&SessionEvent::SummaryWritten {
                path: "/home/user/notes/Algorithms 101/2026-10-22-ders.md".to_string()
            }),
            Some((
                "Lecture notes saved".to_string(),
                "2026-10-22-ders.md".to_string()
            ))
        );
    }

    #[test]
    fn summary_failed_notification_carries_the_error() {
        assert_eq!(
            notification_for(&SessionEvent::SummaryFailed {
                error: "network error".to_string()
            }),
            Some(("Summary failed".to_string(), "network error".to_string()))
        );
    }

    #[test]
    fn other_events_do_not_notify() {
        assert_eq!(
            notification_for(&SessionEvent::Partial {
                text: "hello".to_string()
            }),
            None
        );
        assert_eq!(notification_for(&SessionEvent::Level { rms: 0.5 }), None);
    }
}
