//! 프론트가 호출하는 Tauri 커맨드.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::State;

use crate::core::file_type::FileKind;
use crate::core::fs_port::{FileSystem, RealFs};
use crate::core::job::{JobId, JobRequest, JobStatus};
use crate::core::output::resolve_output_path;
use crate::core::progress::{heartbeat_percent, Heartbeat, CONVERT_STARTED_PERCENT};
use crate::core::runtime::assets::H2O_VERSION;
use crate::core::runtime::plan::{ExtensionState, RuntimeStatus};
use crate::core::settings::{parse_settings, to_json, Settings};
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

/// 상태 조회는 변환과 같은 프로필 잠금을 기다린다 — 대용량 변환 중에는 몇 분이 걸릴 수
/// 있어 메인 스레드에서 돌리면 그동안 창이 통째로 얼어붙는다. 그래서 async 다.
#[tauri::command]
pub async fn get_runtime_status(
    state: State<'_, AppState>,
    refresh: bool,
) -> Result<RuntimeStatusView, String> {
    let Some(runtime) = state.runtime.clone() else {
        return Ok(unsupported_view());
    };

    tauri::async_runtime::spawn_blocking(move || {
        let status = runtime.status(refresh)?;
        status_view(&runtime, status)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 상태 + 실행 파일 경로를 프론트가 쓰는 모양으로 묶는다.
fn status_view(
    runtime: &crate::shell::runtime_manager::RuntimeManager,
    status: RuntimeStatus,
) -> Result<RuntimeStatusView, String> {
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

        outcome.and_then(|status| status_view(&runtime, status))
    })
    .await
    .map_err(|error| error.to_string())??;

    Ok(status)
}

/// 지금 설정. 파일이 없거나 깨졌으면 기본값 — 설정 때문에 앱이 막히면 안 된다.
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    load_settings(&state.settings_path)
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    if let Some(parent) = state.settings_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    std::fs::write(&state.settings_path, to_json(&settings)).map_err(|error| error.to_string())
}

fn load_settings(path: &std::path::Path) -> Settings {
    parse_settings(&std::fs::read_to_string(path).unwrap_or_default())
}

/// 폴더 하나에 여러 건을 저장할 때 쓸 산출물 경로를 정한다.
///
/// 저장 대화상자로 사용자가 직접 고른 경로와 달리 여기서는 파일명에 동의를 받은 적이
/// 없다 — 이름 규칙(접미사)과 충돌 규칙은 설정을 따른다.
#[tauri::command]
pub fn plan_output_path(
    state: State<'_, AppState>,
    source: String,
    dir: String,
) -> Result<String, String> {
    let settings = load_settings(&state.settings_path);
    let fs = RealFs;

    let path = resolve_output_path(
        &PathBuf::from(dir),
        &PathBuf::from(source),
        &settings.name_suffix,
        settings.on_conflict,
        &|candidate| fs.exists(candidate),
    );

    Ok(path.to_string_lossy().into_owned())
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

        // 그 다음은 추정치로 막대를 살려 둔다. 100MB 문서는 몇 분씩 걸리는데
        // 그동안 아무것도 안 보내면 사용자는 앱이 멈춘 줄 안다.
        let heartbeat = {
            let reporter = Arc::clone(&reporter);
            let expected = runtime.expected_conversion_time(&input);

            Heartbeat::start(HEARTBEAT_INTERVAL, move |elapsed| {
                if let Err(error) =
                    reporter.report_progress(id, heartbeat_percent(elapsed, expected))
                {
                    eprintln!("진행률 보고 실패: {error}");
                }
            })
        };

        let outcome = runtime.convert_to_pdf(&input, std::path::Path::new(&out_path));
        // 완료·실패보다 먼저 멈춘다 — 늦게 도착한 추정치가 결과를 덮으면 안 된다.
        heartbeat.stop();

        match outcome {
            Ok(note) => {
                // 안내는 완료보다 먼저 — 완료 이벤트를 보고 UI 가 항목을 접을 수 있다.
                if let Some(note) = note {
                    if let Err(error) = reporter.note(id, note) {
                        eprintln!("안내 보고 실패: {error}");
                    }
                }
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

/// 추정 진행률을 보내는 간격. 짧을수록 부드럽지만 웹뷰 렌더가 늘어난다.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);

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
