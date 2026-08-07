pub mod core;
pub mod shell;

use tauri::Manager;

use shell::commands::{cancel_job, list_jobs, start_demo_job};
use shell::{event_sink::TauriEventSink, AppState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let sink = TauriEventSink::new(app.handle().clone());
            app.manage(AppState::new(sink));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_demo_job,
            cancel_job,
            list_jobs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
