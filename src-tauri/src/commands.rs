//! Tauri commands: thin wrappers that move data between the UI and `lma_core`.

use crate::notify;
use crate::state::{
    AppState, ModelProgress, ModelStatus, NO_SESSION, check_start, lock, notification_for,
    notification_is_silent,
};
use chrono::Local;
use lma_core::capture::{CaptureError, list_output_devices as core_output_devices, system_source};
use lma_core::llm::{LlmClient, validate_api_base_url};
use lma_core::screenshot::capture_monitor_full;
use lma_core::session::{LiveTriggers, Session, SessionConfig, SessionError, SessionEvent, Sink};
use lma_core::settings::{
    Region, Settings, get_api_key, save as save_settings_file, set_api_key as store_api_key,
};
use lma_core::stt::{
    EarshotVad, SttError, WhisperTranscriber, download_model as download_whisper_model,
    model_file_name, valid_model_name,
};
use lma_core::vault::{allocate_session, list_courses as list_vault_courses};
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use ts_rs::TS;

const INVALID_HOTKEY: &str = "Invalid hotkey";
const SAVE_FAILED: &str = "Could not save settings";
const API_KEY_MISSING: &str = "API key missing";
const INVALID_API_URL: &str = "Invalid API URL";
const INVALID_MODEL_NAME: &str = "Invalid Whisper model name";
const NOTES_NOT_FOUND: &str = "Notes folder not found";
const EXPLANATION_TOO_LONG: &str = "Explanation language is too long";
const SCREEN_RECORDING_PERMISSION: &str = "Screen Recording permission required";
const MODEL_LOAD_FAILED: &str = "Whisper model failed to load — download it again in Settings";

fn stored_api_key() -> Option<String> {
    get_api_key().ok().flatten().filter(|key| !key.is_empty())
}

fn llm_from(settings: &Settings) -> Option<Arc<LlmClient>> {
    // An invalid URL means no assist, exactly as when the key is missing.
    if validate_api_base_url(&settings.api_base_url).is_err() {
        return None;
    }
    stored_api_key()
        .map(|key| Arc::new(LlmClient::new(&settings.api_base_url, key, &settings.model)))
}

/// A missing Screen Recording permission has its own recovery path in the UI.
fn start_error(error: SessionError) -> String {
    match error {
        SessionError::Source(CaptureError::PermissionDenied) => {
            SCREEN_RECORDING_PERMISSION.to_string()
        }
        other => other.to_string(),
    }
}

/// A corrupt model is fixed by downloading it again, so the message says so.
fn load_error(error: SttError) -> String {
    match error {
        SttError::ModelLoadFailed => MODEL_LOAD_FAILED.to_string(),
        other => other.to_string(),
    }
}

pub fn is_running(app: &AppHandle) -> bool {
    lock(&app.state::<AppState>().gate)
        .require_running()
        .is_ok()
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    lock(&app.state::<AppState>().settings).clone()
}

/// The webview is untrusted: every value that later reaches the network or the
/// filesystem is checked here, before it is stored.
#[tauri::command]
pub fn save_settings(app: AppHandle, mut settings: Settings) -> Result<(), String> {
    if validate_api_base_url(&settings.api_base_url).is_err() {
        return Err(INVALID_API_URL.to_string());
    }
    if !valid_model_name(&settings.whisper_model) {
        return Err(INVALID_MODEL_NAME.to_string());
    }
    settings.explanation_language = settings.explanation_language.trim().to_string();
    if settings.explanation_language.chars().count() > 40 {
        return Err(EXPLANATION_TOO_LONG.to_string());
    }
    if let Some(notes) = &settings.notes_dir {
        let canonical = notes
            .canonicalize()
            .ok()
            .filter(|path| path.is_dir())
            .ok_or_else(|| NOTES_NOT_FOUND.to_string())?;
        settings.notes_dir = Some(canonical);
    }
    let state = app.state::<AppState>();
    let old = lock(&state.settings).clone();
    // Persist first: a hotkey switch that fails must leave file, state and live shortcut unchanged.
    save_settings_file(&state.settings_path, &settings).map_err(|_| SAVE_FAILED.to_string())?;
    if settings.hotkey != old.hotkey {
        unregister_hotkey(&app, &old.hotkey);
        if register_hotkey(&app, &settings.hotkey).is_err() {
            let _ = save_settings_file(&state.settings_path, &old);
            if register_hotkey(&app, &old.hotkey).is_err() {
                eprintln!("could not restore the previous hotkey");
            }
            return Err(INVALID_HOTKEY.to_string());
        }
    }
    let live = LiveTriggers {
        names: settings.name_variants.clone(),
        keywords: settings.keywords.clone(),
        general_questions: settings.general_questions,
        name_bias: settings.name_bias,
        explanation_language: settings.explanation_language.clone(),
    };
    *lock(&state.settings) = settings;
    if let Some(session) = lock(&state.session).as_ref() {
        session.update_triggers(live);
    }
    Ok(())
}

#[tauri::command]
pub fn has_api_key() -> bool {
    stored_api_key().is_some()
}

#[tauri::command]
pub fn set_api_key(key: String) -> Result<(), String> {
    store_api_key(&key).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_courses(app: AppHandle) -> Result<Vec<String>, String> {
    let state = app.state::<AppState>();
    let notes = lock(&state.settings)
        .notes_dir
        .clone()
        .ok_or_else(|| crate::state::NOTES_MISSING.to_string())?;
    list_vault_courses(&notes).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn start_session(app: AppHandle, course: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    lock(&state.gate).try_begin()?;
    if let Err(error) = start_session_inner(&app, &state, course).await {
        let _ = lock(&state.gate).end();
        return Err(error);
    }
    Ok(())
}

async fn start_session_inner(
    app: &AppHandle,
    state: &AppState,
    course: String,
) -> Result<(), String> {
    let settings = lock(&state.settings).clone();
    let model_path = state
        .models_dir
        .join(model_file_name(&settings.whisper_model));
    let notes = check_start(&settings, model_path.is_file())?;
    let llm = llm_from(&settings);

    let language = settings.language.clone();
    let stt = state
        .runtime
        .spawn_blocking(move || WhisperTranscriber::load(&model_path, &language))
        .await
        .map_err(|_| "Could not load the Whisper model".to_string())?
        .map_err(load_error)?;
    let source =
        system_source(settings.audio_device.as_deref()).map_err(|error| error.to_string())?;
    let date = Local::now().date_naive();
    let files = allocate_session(&notes, &course, date).map_err(|error| error.to_string())?;

    let cfg = SessionConfig {
        course: course.clone(),
        files: files.clone(),
        date,
        names: settings.name_variants.clone(),
        keywords: settings.keywords.clone(),
        general_questions: settings.general_questions,
        name_bias: settings.name_bias,
        screenshot: (
            settings.screenshot_mode.clone(),
            settings.monitor.clone(),
            settings.region.clone(),
        ),
        explanation_language: settings.explanation_language.clone(),
        no_audio_after: Duration::from_secs(10),
    };

    let sink_app = app.clone();
    let summary = Arc::clone(&state.summary);
    let sink: Sink = Arc::new(move |event: SessionEvent| {
        let _ = sink_app.emit("session-event", &event);
        if let Some((title, body)) = notification_for(&event) {
            notify::show(&sink_app, &title, &body, notification_is_silent(&event));
        }
        if matches!(
            event,
            SessionEvent::SummaryWritten { .. } | SessionEvent::SummaryFailed { .. }
        ) {
            summary.complete();
        }
    });

    let session = Session::start(
        cfg,
        source,
        Box::new(stt),
        Box::new(EarshotVad::new()),
        llm,
        state.runtime.clone(),
        sink,
    )
    .map_err(start_error)?;

    *lock(&state.session) = Some(session);
    *lock(&state.last) = Some((files, course, date));
    Ok(())
}

#[tauri::command]
pub async fn stop_session(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let Some(session) = lock(&state.session).take() else {
        return Err(NO_SESSION.to_string());
    };
    // The gate result is reported after stopping, never instead of stopping.
    let gate = lock(&state.gate).end();
    // Set before `stop`, so the summary's completion cannot race past a Quit waiting on it.
    state.summary.begin();
    let stopped = match state.runtime.spawn_blocking(move || session.stop()).await {
        Ok(Ok(())) => return gate,
        Ok(Err(error)) => error.to_string(),
        Err(_) => "Could not stop the session".to_string(),
    };
    state.summary.complete(); // no summary will arrive
    Err(stopped)
}

#[tauri::command]
pub fn ask_now(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    lock(&state.gate).require_running()?;
    let session = lock(&state.session);
    let Some(session) = session.as_ref() else {
        return Err(NO_SESSION.to_string());
    };
    session.ask_now();
    Ok(())
}

#[tauri::command]
pub fn capture_and_ask(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    lock(&state.gate).require_running()?;
    let session = lock(&state.session);
    let Some(session) = session.as_ref() else {
        return Err(NO_SESSION.to_string());
    };
    session.capture_and_ask();
    Ok(())
}

#[tauri::command]
pub async fn retry_summary(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    // A live session's summary arrives at Stop; retrying now would write a premature one.
    lock(&state.gate).require_idle()?;
    let Some((files, course, date)) = lock(&state.last).clone() else {
        return Err("No transcript to summarize".to_string());
    };
    let settings = lock(&state.settings).clone();
    if validate_api_base_url(&settings.api_base_url).is_err() {
        return Err(INVALID_API_URL.to_string());
    }
    let key = stored_api_key().ok_or_else(|| API_KEY_MISSING.to_string())?;
    let llm = LlmClient::new(&settings.api_base_url, key, &settings.model);

    let result = Session::retry_summary(&files, &course, date, &llm).await;
    let event = match &result {
        Ok(()) => SessionEvent::SummaryWritten {
            path: files.summary.display().to_string(),
        },
        Err(error) => SessionEvent::SummaryFailed {
            error: error.to_string(),
        },
    };
    let _ = app.emit("session-event", &event);
    result.map_err(|error| error.to_string())
}

#[tauri::command]
pub fn model_status(app: AppHandle) -> ModelStatus {
    let state = app.state::<AppState>();
    let model = lock(&state.settings).whisper_model.clone();
    let installed = state.models_dir.join(model_file_name(&model)).is_file();
    ModelStatus { installed, model }
}

#[tauri::command]
pub async fn download_model(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let model = lock(&state.settings).whisper_model.clone();
    // At most ~10 progress events per second; the loop calls this per network chunk.
    let last = std::sync::Mutex::new(None::<Instant>);
    let progress = |done: u64, total: u64| {
        let mut last = lock(&last);
        let due = last.is_none_or(|at: Instant| at.elapsed() >= Duration::from_millis(100));
        if due {
            *last = Some(Instant::now());
            let _ = app.emit("model-progress", ModelProgress { done, total });
        }
    };
    // Always download: the file may exist but be corrupt, and Settings is the recovery path.
    download_whisper_model(&model, &state.models_dir, true, progress)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// The one deep link the app may open; fixed, so no webview value reaches the command.
#[tauri::command]
pub fn open_screen_recording_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn()
            .map(|_| ())
            .map_err(|_| "Could not open System Settings".to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Not supported".to_string())
    }
}

#[tauri::command]
pub fn list_monitors() -> Result<Vec<String>, String> {
    lma_core::screenshot::list_monitors().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_output_devices() -> Vec<String> {
    core_output_devices()
}

#[tauri::command]
// async: building a window inside a sync command deadlocks on Windows.
pub async fn open_region_picker(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("region") {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    let state = app.state::<AppState>();
    let selected = lock(&state.settings).monitor.clone();
    let monitors = app
        .available_monitors()
        .map_err(|_| "Could not list monitors".to_string())?;
    let monitor = selected
        .and_then(|name| monitors.iter().find(|m| m.name() == Some(&name)).cloned())
        .or_else(|| app.primary_monitor().ok().flatten())
        .or_else(|| monitors.first().cloned())
        .ok_or_else(|| "No monitor found".to_string())?;

    let scale = monitor.scale_factor();
    let position = monitor.position();
    tauri::WebviewWindowBuilder::new(
        &app,
        "region",
        tauri::WebviewUrl::App("index.html#/region".into()),
    )
    .title("Select screenshot region — drag a rectangle, Enter to save, Esc to cancel")
    .decorations(true)
    .resizable(true)
    .maximized(true)
    // Hidden until the page has the picture, or the capture would include this window.
    .visible(false)
    .position(f64::from(position.x) / scale, f64::from(position.y) / scale)
    .build()
    .map_err(|_| "Could not open the region picker".to_string())?;
    Ok(())
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct RegionSnapshot {
    pub image_base64: String,
    pub media_type: String,
    pub monitor_width: u32,
    pub monitor_height: u32,
}

/// The chosen monitor's picture for the region picker, in memory only.
#[tauri::command]
pub async fn region_snapshot(app: AppHandle) -> Result<RegionSnapshot, String> {
    let monitor = lock(&app.state::<AppState>().settings).monitor.clone();
    let (image, width, height) =
        capture_monitor_full(monitor.as_deref()).map_err(|error| error.to_string())?;
    Ok(RegionSnapshot {
        image_base64: image.base64,
        media_type: image.media_type,
        monitor_width: width,
        monitor_height: height,
    })
}

#[tauri::command]
pub fn save_region(app: AppHandle, region: Region) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut settings = lock(&state.settings).clone();
    settings.region = Some(region);
    save_settings_file(&state.settings_path, &settings).map_err(|_| SAVE_FAILED.to_string())?;
    *lock(&state.settings) = settings;
    if let Some(window) = app.get_webview_window("region") {
        let _ = window.close();
    }
    Ok(())
}

/// Registers `hotkey` for "Ask now"; errors never include the key or a response body.
pub fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let shortcut: Shortcut = hotkey.parse().map_err(|_| INVALID_HOTKEY.to_string())?;
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = ask_now(app.clone());
            }
        })
        .map_err(|_| INVALID_HOTKEY.to_string())
}

fn unregister_hotkey(app: &AppHandle, hotkey: &str) {
    if let Ok(shortcut) = hotkey.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(shortcut);
    }
}

/// Stops a running session, waits (off the UI thread) up to 120 s for its summary, then exits.
pub fn quit(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if is_running(&app) {
            let _ = stop_session(app.clone()).await;
        }
        let summary = Arc::clone(&app.state::<AppState>().summary);
        let _ = tauri::async_runtime::spawn_blocking(move || {
            summary.wait(Duration::from_secs(120));
        })
        .await;
        app.exit(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_denied_gets_the_screen_recording_message() {
        assert_eq!(
            start_error(SessionError::Source(CaptureError::PermissionDenied)),
            SCREEN_RECORDING_PERMISSION
        );
    }

    #[test]
    fn other_capture_errors_keep_their_message() {
        assert_eq!(
            start_error(SessionError::Source(CaptureError::NoDevice)),
            "no audio output device"
        );
    }

    #[test]
    fn model_load_failure_points_at_the_settings_download() {
        assert_eq!(load_error(SttError::ModelLoadFailed), MODEL_LOAD_FAILED);
        assert_eq!(load_error(SttError::ModelMissing), "model file missing");
    }
}
