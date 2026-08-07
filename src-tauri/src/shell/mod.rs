//! Tauri 셸 — 코어를 앱 런타임(상태·커맨드·이벤트)에 붙이는 얇은 계층.

pub mod commands;
pub mod event_sink;

use std::sync::Arc;

use crate::core::events::JobReporter;
use crate::core::job::JobQueue;
use event_sink::TauriEventSink;

/// Tauri `manage` 로 공유되는 앱 상태.
pub struct AppState {
    pub reporter: Arc<JobReporter<TauriEventSink>>,
}

impl AppState {
    pub fn new(sink: TauriEventSink) -> Self {
        Self {
            reporter: Arc::new(JobReporter::new(Arc::new(JobQueue::new()), sink)),
        }
    }
}
