pub mod core;
pub mod shell;

use tauri::Manager;

use shell::commands::{
    cancel_job, convert_hwp, get_runtime_status, get_settings, install_runtime, list_jobs,
    plan_output_path, save_settings,
};
use shell::{event_sink::TauriEventSink, AppState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let sink = TauriEventSink::new(app.handle().clone());
            // 런타임은 로컬 데이터 디렉토리에 둔다 (Windows 로밍 프로필 금지).
            let data_dir = app.path().app_local_data_dir()?;
            app.manage(AppState::new(sink, &data_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cancel_job,
            list_jobs,
            get_runtime_status,
            install_runtime,
            plan_output_path,
            get_settings,
            save_settings,
            convert_hwp
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
