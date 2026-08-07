//! 코어의 [`EventSink`] 를 Tauri 이벤트로 연결한다.

use tauri::{AppHandle, Emitter};

use crate::core::events::{EventSink, JobEvent, JOB_EVENT};

pub struct TauriEventSink {
    app: AppHandle,
}

impl TauriEventSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl EventSink for TauriEventSink {
    fn emit(&self, event: &JobEvent) -> Result<(), String> {
        self.app
            .emit(JOB_EVENT, event)
            .map_err(|error| error.to_string())
    }
}
