//! OS notifications. Best effort: a failed notification never disturbs the session.

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn show(app: &AppHandle, title: &str, body: &str, silent: bool) {
    let builder = app.notification().builder().title(title).body(body);
    let builder = if silent { builder.silent() } else { builder };
    let _ = builder.show();
}

/// Ask once at startup: macOS may prompt here. Errors are ignored, notifications stay best effort.
pub fn request_permission(app: &AppHandle) {
    let _ = app.notification().permission_state();
    let _ = app.notification().request_permission();
}
