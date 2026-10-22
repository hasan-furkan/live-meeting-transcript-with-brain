//! Tauri commands: thin wrappers that move data between the UI and `lma_core`.

use crate::notify;
use crate::state::{
    AppState, ModelProgress, ModelStatus, NO_SESSION, check_start, lock, notification_for,
};
use chrono::Local;
use lma_core::capture::{list_output_devices as core_output_devices, system_source};
use lma_core::llm::LlmClient;
use lma_core::session::{Session, SessionConfig, SessionEvent, Sink};
use lma_core::settings::{
    Region, Settings, get_api_key, save as save_settings_file, set_api_key as store_api_key,
};
use lma_core::stt::{
    EarshotVad, WhisperTranscriber, download_model as download_whisper_model, model_file_name,
};
use lma_core::vault::{allocate_session, list_courses as list_vault_courses};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const INVALID_HOTKEY: &str = "Invalid hotkey";
const SAVE_FAILED: &str = "Could not save settings";
const API_KEY_MISSING: &str = "API key missing";

fn stored_api_key() -> Option<String> {
    get_api_key().ok().flatten().filter(|key| !key.is_empty())
}

fn llm_from(settings: &Settings) -> Option<Arc<LlmClient>> {
    stored_api_key()
        .map(|key| Arc::new(LlmClient::new(&settings.api_base_url, key, &settings.model)))
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

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let state = app.state::<AppState>();
    let old = lock(&state.settings).hotkey.clone();
    if settings.hotkey != old {
        unregister_hotkey(&app, &old);
        if register_hotkey(&app, &settings.hotkey).is_err() {
            let _ = register_hotkey(&app, &old);
            return Err(INVALID_HOTKEY.to_string());
        }
    }
    save_settings_file(&state.settings_path, &settings).map_err(|_| SAVE_FAILED.to_string())?;
    *lock(&state.settings) = settings;
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
        .map_err(|error| error.to_string())?;
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
        no_audio_after: Duration::from_secs(10),
    };

    let sink_app = app.clone();
    let sink: Sink = Arc::new(move |event: SessionEvent| {
        let _ = sink_app.emit("session-event", &event);
        if let Some((title, body)) = notification_for(&event) {
            notify::show(&sink_app, &title, &body);
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
    .map_err(|error| error.to_string())?;

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
    lock(&state.gate).end()?;
    state
        .runtime
        .spawn_blocking(move || session.stop())
        .await
        .map_err(|_| "Could not stop the session".to_string())?
        .map_err(|error| error.to_string())
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
pub async fn retry_summary(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let Some((files, course, date)) = lock(&state.last).clone() else {
        return Err("No transcript to summarize".to_string());
    };
    let settings = lock(&state.settings).clone();
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
    download_whisper_model(&model, &state.models_dir, progress)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
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
pub fn open_region_picker(app: AppHandle) -> Result<(), String> {
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
    let size = monitor.size();
    tauri::WebviewWindowBuilder::new(
        &app,
        "region",
        tauri::WebviewUrl::App("index.html#/region".into()),
    )
    .transparent(true)
    .decorations(false)
    .always_on_top(true)
    .resizable(false)
    .position(f64::from(position.x) / scale, f64::from(position.y) / scale)
    .inner_size(
        f64::from(size.width) / scale,
        f64::from(size.height) / scale,
    )
    .build()
    .map_err(|_| "Could not open the region picker".to_string())?;
    Ok(())
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

/// Stops a running session before leaving, then exits.
pub fn quit(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if is_running(&app) {
            let _ = stop_session(app.clone()).await;
        }
        app.exit(0);
    });
}
