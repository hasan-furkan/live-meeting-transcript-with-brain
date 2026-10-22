#![forbid(unsafe_code)]

mod commands;
mod notify;
mod state;

use state::{AppState, SessionGate, SummaryTracker};
use std::sync::{Arc, Mutex};
use tauri::menu::MenuBuilder;
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Idle: show the window so the user can pick a course. Running: stop.
fn toggle_session(app: &tauri::AppHandle) {
    if commands::is_running(app) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = commands::stop_session(app).await;
        });
    } else {
        show_main(app);
    }
}

fn setup_state(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let settings_path = app.path().app_config_dir()?.join("settings.json");
    let settings = lma_core::settings::load(&settings_path);
    let models_dir = app.path().app_data_dir()?.join("models");
    let _ = std::fs::create_dir_all(&models_dir);

    app.manage(AppState {
        settings: Mutex::new(settings.clone()),
        settings_path,
        models_dir,
        gate: Mutex::new(SessionGate::new()),
        session: Mutex::new(None),
        last: Mutex::new(None),
        summary: Arc::new(SummaryTracker::new()),
        runtime: tauri::async_runtime::handle().inner().clone(),
    });

    let _ = commands::register_hotkey(app.handle(), &settings.hotkey);
    notify::request_permission(app.handle());
    Ok(())
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| std::io::Error::other("missing window icon"))?;
    let menu = MenuBuilder::new(app)
        .text("show", "Show")
        .text("toggle", "Start/Stop")
        .separator()
        .text("quit", "Quit")
        .build()?;
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main(app),
            "toggle" => toggle_session(app),
            "quit" => commands::quit(app),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::has_api_key,
            commands::set_api_key,
            commands::list_courses,
            commands::start_session,
            commands::stop_session,
            commands::ask_now,
            commands::capture_and_ask,
            commands::retry_summary,
            commands::model_status,
            commands::download_model,
            commands::open_screen_recording_settings,
            commands::list_monitors,
            commands::list_output_devices,
            commands::open_region_picker,
            commands::region_snapshot,
            commands::save_region,
        ])
        .setup(|app| {
            setup_state(app)?;
            setup_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window keeps a running session alive in the tray.
            if window.label() == "main"
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("error while running app: {e}");
    }
}
