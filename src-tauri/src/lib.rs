#![forbid(unsafe_code)]

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(e) = tauri::Builder::default().run(tauri::generate_context!()) {
        eprintln!("error while running app: {e}");
    }
}
