//! 프론트가 호출하는 Tauri 커맨드.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::State;

use crate::core::file_type::FileKind;
use crate::core::job::{JobId, JobRequest, JobStatus};
use crate::shell::AppState;

/// 데모 카운터 한 틱의 간격과 총 단계 수 (2초 동안 5%씩 진행).
const DEMO_TICK: Duration = Duration::from_millis(100);
const DEMO_STEPS: u8 = 20;
const DEMO_STEP_PERCENT: u8 = 5;

/// 프론트에 넘기는 작업 스냅샷 (코어 타입을 그대로 노출하지 않는다).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: JobId,
    pub source: String,
    pub status: String,
    pub progress: u8,
}

/// 진행 이벤트 브리지를 눈으로 확인하기 위한 데모 작업.
/// 실제 변환 파이프라인(Phase 2)이 붙으면 대체된다.
#[tauri::command]
pub fn start_demo_job(state: State<'_, AppState>) -> Result<JobId, String> {
    let reporter = Arc::clone(&state.reporter);

    let id = reporter
        .enqueue(JobRequest {
            source: PathBuf::from("데모 카운터"),
            target: FileKind::Pdf,
        })
        .map_err(|error| error.to_string())?;
    reporter.claim_next();

    std::thread::spawn(move || {
        for step in 1..=DEMO_STEPS {
            std::thread::sleep(DEMO_TICK);

            if reporter.is_cancel_requested(id) {
                if let Err(error) = reporter.mark_cancelled(id) {
                    eprintln!("데모 작업 취소 확정 실패: {error}");
                }
                return;
            }

            if let Err(error) = reporter.report_progress(id, step * DEMO_STEP_PERCENT) {
                eprintln!("데모 진행률 보고 실패: {error}");
                return;
            }
        }

        if let Err(error) = reporter.complete(id) {
            eprintln!("데모 작업 완료 처리 실패: {error}");
        }
    });

    Ok(id)
}

#[tauri::command]
pub fn cancel_job(state: State<'_, AppState>, id: JobId) -> Result<(), String> {
    state
        .reporter
        .cancel(id)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_jobs(state: State<'_, AppState>) -> Vec<JobView> {
    state
        .reporter
        .snapshot()
        .into_iter()
        .map(|job| JobView {
            id: job.id,
            source: job.request.source.to_string_lossy().into_owned(),
            status: status_label(&job.status),
            progress: job.progress,
        })
        .collect()
}

fn status_label(status: &JobStatus) -> String {
    match status {
        JobStatus::Queued => "queued".to_string(),
        JobStatus::Running => "running".to_string(),
        JobStatus::Cancelling => "cancelling".to_string(),
        JobStatus::Cancelled => "cancelled".to_string(),
        JobStatus::Completed => "completed".to_string(),
        JobStatus::Failed(_) => "failed".to_string(),
    }
}
