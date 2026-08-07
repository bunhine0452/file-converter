//! 프론트가 호출하는 Tauri 커맨드.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::core::file_type::FileKind;
use crate::core::job::{JobId, JobRequest, JobStatus};
use crate::core::runtime::assets::H2O_VERSION;
use crate::core::runtime::plan::{ExtensionState, RuntimeStatus};
use crate::shell::runtime_manager::InstallEvent;
use crate::shell::AppState;

/// 프론트에 넘기는 작업 스냅샷 (코어 타입을 그대로 노출하지 않는다).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: JobId,
    pub source: String,
    pub status: String,
    pub progress: u8,
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

// ── 런타임 준비와 변환 ────────────────────────────────────────────

/// 프론트에 넘기는 런타임 상태.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatusView {
    /// ready | needsLibreOffice | needsJre | needsExtension | unsupported
    pub state: String,
    pub version: Option<String>,
    pub exe_path: Option<String>,
    /// 앱이 직접 설치한 LibreOffice 인가
    pub managed: bool,
}

#[tauri::command]
pub fn get_runtime_status(
    state: State<'_, AppState>,
    refresh: bool,
) -> Result<RuntimeStatusView, String> {
    let Some(runtime) = state.runtime.clone() else {
        return Ok(unsupported_view());
    };

    let status = runtime.status(refresh)?;
    let exe = runtime.soffice()?;

    Ok(RuntimeStatusView {
        state: runtime_state_label(&status).to_string(),
        version: status.libreoffice.as_ref().map(|lo| lo.version.to_string()),
        exe_path: exe.map(|info| info.exe.display().to_string()),
        managed: status.libreoffice.map(|lo| lo.managed).unwrap_or(false),
    })
}

/// 부족한 런타임을 내려받아 설치한다. 진행 상황은 채널로 흘려보낸다.
#[tauri::command]
pub async fn install_runtime(
    state: State<'_, AppState>,
    on_event: tauri::ipc::Channel<InstallEvent>,
) -> Result<RuntimeStatusView, String> {
    let Some(runtime) = state.runtime.clone() else {
        return Ok(unsupported_view());
    };

    // 수백 MB 다운로드가 UI 스레드를 잡으면 안 된다.
    let status = tauri::async_runtime::spawn_blocking(move || {
        let outcome = runtime.install(
            &mut |event| {
                if let Err(error) = on_event.send(event) {
                    eprintln!("설치 진행 이벤트 전송 실패: {error}");
                }
            },
            &|| false,
        );

        outcome.map(|status| {
            let exe = runtime.soffice().ok().flatten();
            RuntimeStatusView {
                state: runtime_state_label(&status).to_string(),
                version: status.libreoffice.as_ref().map(|lo| lo.version.to_string()),
                exe_path: exe.map(|info| info.exe.display().to_string()),
                managed: status.libreoffice.map(|lo| lo.managed).unwrap_or(false),
            }
        })
    })
    .await
    .map_err(|error| error.to_string())??;

    Ok(status)
}

/// HWP/HWPX 한 건을 PDF 로 변환한다. 진행 상황은 기존 작업 이벤트로 나간다.
#[tauri::command]
pub fn convert_hwp(
    state: State<'_, AppState>,
    source: String,
    out_path: String,
) -> Result<JobId, String> {
    let runtime = state
        .runtime
        .clone()
        .ok_or_else(|| "이 플랫폼에서는 변환을 지원하지 않습니다".to_string())?;

    let input = PathBuf::from(&source);
    // 프론트의 확장자 검사만 믿지 않는다 — 드롭 경로에는 디렉토리도 섞여 온다.
    if !input.is_file() {
        return Err("파일이 아닙니다".to_string());
    }

    let reporter = Arc::clone(&state.reporter);
    let id = reporter
        .enqueue(JobRequest {
            source: input.clone(),
            target: FileKind::Pdf,
        })
        .map_err(|error| error.to_string())?;
    reporter.claim_next();

    std::thread::spawn(move || {
        // soffice 는 중간 진행률을 주지 않는다 — 시작했다는 사실만 알린다.
        if let Err(error) = reporter.report_progress(id, CONVERT_STARTED_PERCENT) {
            eprintln!("진행률 보고 실패: {error}");
        }

        match runtime.convert_to_pdf(&input, std::path::Path::new(&out_path)) {
            Ok(()) => {
                if let Err(error) = reporter.complete(id) {
                    eprintln!("완료 처리 실패: {error}");
                }
            }
            Err(message) => {
                if let Err(error) = reporter.fail(id, message) {
                    eprintln!("실패 처리 실패: {error}");
                }
            }
        }
    });

    Ok(id)
}

/// 변환이 시작됐음을 알리는 최소 진행률 (soffice 는 중간 보고를 하지 않는다).
const CONVERT_STARTED_PERCENT: u8 = 5;

fn unsupported_view() -> RuntimeStatusView {
    RuntimeStatusView {
        state: "unsupported".to_string(),
        version: None,
        exe_path: None,
        managed: false,
    }
}

fn runtime_state_label(status: &RuntimeStatus) -> &'static str {
    if status.libreoffice.is_none() {
        return "needsLibreOffice";
    }
    if status.java_home.is_none() {
        return "needsJre";
    }
    match &status.extension {
        ExtensionState::Registered { version } if version == H2O_VERSION => "ready",
        _ => "needsExtension",
    }
}
