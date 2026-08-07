//! Tauri 셸 — 코어를 앱 런타임(상태·커맨드·이벤트)에 붙이는 얇은 계층.

pub mod commands;
pub mod event_sink;
pub mod runtime_manager;

use std::sync::Arc;

use crate::core::events::JobReporter;
use crate::core::fs_port::RealFs;
use crate::core::job::JobQueue;
use crate::core::runtime::assets::Platform;
use crate::core::runtime::download::ReqwestDownloader;
use crate::core::runtime::real_installer::RealInstaller;
use crate::core::soffice::probe::RealProbe;
use crate::core::soffice::runner::RealRunner;
use event_sink::TauriEventSink;
use runtime_manager::{RuntimeManager, RuntimePaths};

/// Tauri `manage` 로 공유되는 앱 상태.
pub struct AppState {
    pub reporter: Arc<JobReporter<TauriEventSink>>,
    /// 지원하지 않는 플랫폼이면 None — 앱은 뜨되 변환 기능만 막힌다.
    pub runtime: Option<Arc<RuntimeManager>>,
}

impl AppState {
    pub fn new(sink: TauriEventSink, app_local_data_dir: &std::path::Path) -> Self {
        Self {
            reporter: Arc::new(JobReporter::new(Arc::new(JobQueue::new()), sink)),
            runtime: build_runtime(app_local_data_dir),
        }
    }
}

fn build_runtime(app_local_data_dir: &std::path::Path) -> Option<Arc<RuntimeManager>> {
    let platform = Platform::host()?;
    let paths = match RuntimePaths::new(app_local_data_dir, platform) {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("런타임 경로를 만들지 못했습니다: {error}");
            return None;
        }
    };

    let runner = Arc::new(RealRunner);
    let fs = Arc::new(RealFs);

    Some(Arc::new(RuntimeManager::new(
        // 앱이 설치한 LibreOffice 를 최우선 후보로 알려준다.
        Arc::new(RealProbe::new(Some(paths.libreoffice.clone()), None)),
        runner.clone(),
        fs.clone(),
        Arc::new(ReqwestDownloader),
        Arc::new(RealInstaller::new(runner, fs, platform.os)),
        paths,
        platform,
    )))
}
