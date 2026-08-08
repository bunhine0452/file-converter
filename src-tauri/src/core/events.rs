//! Rust→프론트 진행 이벤트 브리지.
//!
//! 코어는 Tauri 를 모른다 — [`EventSink`] 뒤로 감춰 두고 셸에서 실제 구현을 주입한다.
//! 그래야 이벤트 흐름을 Tauri 런타임 없이 단위 테스트할 수 있다.

use std::sync::Arc;

use serde::Serialize;

use crate::core::job::{Job, JobError, JobId, JobQueue, JobRequest, JobStatus, PROGRESS_MAX};

/// 프론트가 구독할 이벤트 이름.
pub const JOB_EVENT: &str = "job://event";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum JobEvent {
    #[serde(rename_all = "camelCase")]
    Queued { id: JobId, source: String },
    #[serde(rename_all = "camelCase")]
    Progress { id: JobId, progress: u8 },
    #[serde(rename_all = "camelCase")]
    Completed { id: JobId },
    #[serde(rename_all = "camelCase")]
    Failed { id: JobId, message: String },
    /// 변환은 진행하되 사용자에게 함께 보여줄 안내 (프리플라이트 경고 등).
    #[serde(rename_all = "camelCase")]
    Note { id: JobId, message: String },
    #[serde(rename_all = "camelCase")]
    Cancelling { id: JobId },
    #[serde(rename_all = "camelCase")]
    Cancelled { id: JobId },
}

/// 이벤트를 실제로 내보내는 곳 (Tauri `AppHandle`, 테스트용 기록기 등).
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &JobEvent) -> Result<(), String>;
}

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error(transparent)]
    Job(#[from] JobError),
    #[error("진행 이벤트 발행 실패: {0}")]
    Emit(String),
}

/// 큐 상태 변경과 이벤트 발행을 한 곳에서 묶는다 — 둘이 어긋나면 UI 가 거짓말을 하게 된다.
pub struct JobReporter<S: EventSink> {
    queue: Arc<JobQueue>,
    sink: S,
}

impl<S: EventSink> JobReporter<S> {
    pub fn new(queue: Arc<JobQueue>, sink: S) -> Self {
        Self { queue, sink }
    }

    pub fn enqueue(&self, request: JobRequest) -> Result<JobId, ReportError> {
        let source = request.source.to_string_lossy().into_owned();
        let id = self.queue.enqueue(request);
        self.emit(&JobEvent::Queued { id, source })?;

        Ok(id)
    }

    /// 워커가 다음 작업을 집어 실행 상태로 넘긴다.
    pub fn claim_next(&self) -> Option<Job> {
        self.queue.claim_next()
    }

    /// 진행률을 갱신하고, 값이 실제로 바뀐 경우에만 이벤트를 발행한다.
    pub fn report_progress(&self, id: JobId, progress: u8) -> Result<(), ReportError> {
        let current = self.queue.get(id).ok_or(JobError::NotFound(id))?;
        let progress = progress.min(PROGRESS_MAX);

        // 같은 값을 반복 발행하면 웹뷰가 무의미한 렌더로 밀린다.
        if current.progress == progress {
            return Ok(());
        }

        self.queue.set_progress(id, progress)?;
        self.emit(&JobEvent::Progress { id, progress })
    }

    /// 진행을 막지 않는 안내를 알린다 — 상태도 진행률도 바꾸지 않는 순수 통지다.
    pub fn note(&self, id: JobId, message: impl Into<String>) -> Result<(), ReportError> {
        // 존재하지 않는 작업의 안내는 UI 가 붙일 곳이 없다.
        if self.queue.get(id).is_none() {
            return Err(JobError::NotFound(id).into());
        }

        self.emit(&JobEvent::Note {
            id,
            message: message.into(),
        })
    }

    pub fn complete(&self, id: JobId) -> Result<(), ReportError> {
        self.queue.complete(id)?;
        self.emit(&JobEvent::Completed { id })
    }

    pub fn fail(&self, id: JobId, message: impl Into<String>) -> Result<(), ReportError> {
        let message = message.into();
        self.queue.fail(id, message.clone())?;
        self.emit(&JobEvent::Failed { id, message })
    }

    pub fn cancel(&self, id: JobId) -> Result<JobStatus, ReportError> {
        let status = self.queue.cancel(id)?;

        match status {
            JobStatus::Cancelled => self.emit(&JobEvent::Cancelled { id })?,
            JobStatus::Cancelling => self.emit(&JobEvent::Cancelling { id })?,
            _ => {}
        }

        Ok(status)
    }

    pub fn mark_cancelled(&self, id: JobId) -> Result<(), ReportError> {
        self.queue.mark_cancelled(id)?;
        self.emit(&JobEvent::Cancelled { id })
    }

    /// 워커 루프가 중단 시점을 판단할 때 쓴다.
    pub fn is_cancel_requested(&self, id: JobId) -> bool {
        self.queue.is_cancel_requested(id)
    }

    pub fn snapshot(&self) -> Vec<Job> {
        self.queue.list()
    }

    fn emit(&self, event: &JobEvent) -> Result<(), ReportError> {
        self.sink.emit(event).map_err(ReportError::Emit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::file_type::FileKind;
    use std::path::PathBuf;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingSink {
        events: Mutex<Vec<JobEvent>>,
        fail_with: Option<String>,
    }

    impl RecordingSink {
        fn failing(message: &str) -> Self {
            Self {
                events: Mutex::new(Vec::new()),
                fail_with: Some(message.to_string()),
            }
        }

        fn events(&self) -> Vec<JobEvent> {
            self.events.lock().unwrap().clone()
        }
    }

    impl EventSink for RecordingSink {
        fn emit(&self, event: &JobEvent) -> Result<(), String> {
            if let Some(message) = &self.fail_with {
                return Err(message.clone());
            }
            self.events.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    /// sink 를 테스트에서 계속 들여다봐야 하므로 Arc 로 공유한다.
    impl EventSink for Arc<RecordingSink> {
        fn emit(&self, event: &JobEvent) -> Result<(), String> {
            self.as_ref().emit(event)
        }
    }

    fn request() -> JobRequest {
        JobRequest {
            source: PathBuf::from("/tmp/계약서.hwp"),
            target: FileKind::Pdf,
        }
    }

    fn reporter() -> (JobReporter<Arc<RecordingSink>>, Arc<RecordingSink>) {
        let sink = Arc::new(RecordingSink::default());
        let queue = Arc::new(JobQueue::new());
        (JobReporter::new(queue, Arc::clone(&sink)), sink)
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 등록하면_queued_이벤트가_발행되고_큐에도_남는다() {
        let (reporter, sink) = reporter();

        let id = reporter.enqueue(request()).expect("등록 성공");

        assert_eq!(
            sink.events(),
            vec![JobEvent::Queued {
                id,
                source: "/tmp/계약서.hwp".to_string(),
            }]
        );
        assert_eq!(reporter.snapshot().len(), 1);
    }

    #[test]
    fn 진행률_보고가_이벤트와_큐_상태에_모두_반영된다() {
        let (reporter, sink) = reporter();
        let id = reporter.enqueue(request()).expect("등록 성공");

        for progress in [10, 55, 90] {
            reporter.report_progress(id, progress).expect("진행률 보고");
        }

        let progress_events: Vec<u8> = sink
            .events()
            .into_iter()
            .filter_map(|event| match event {
                JobEvent::Progress { progress, .. } => Some(progress),
                _ => None,
            })
            .collect();

        assert_eq!(progress_events, vec![10, 55, 90]);
        assert_eq!(reporter.snapshot()[0].progress, 90);
    }

    #[test]
    fn 완료하면_completed_이벤트와_진행률_100_이_된다() {
        let (reporter, sink) = reporter();
        let id = reporter.enqueue(request()).expect("등록 성공");

        reporter.complete(id).expect("완료 처리");

        assert!(sink.events().contains(&JobEvent::Completed { id }));
        let job = &reporter.snapshot()[0];
        assert_eq!(job.progress, 100);
        assert_eq!(job.status, JobStatus::Completed);
    }

    #[test]
    fn 실패하면_사유가_담긴_failed_이벤트가_발행된다() {
        let (reporter, sink) = reporter();
        let id = reporter.enqueue(request()).expect("등록 성공");

        reporter
            .fail(id, "암호가 걸린 문서입니다")
            .expect("실패 처리");

        assert!(sink.events().contains(&JobEvent::Failed {
            id,
            message: "암호가 걸린 문서입니다".to_string(),
        }));
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 같은_진행률을_다시_보고하면_이벤트를_중복_발행하지_않는다() {
        let (reporter, sink) = reporter();
        let id = reporter.enqueue(request()).expect("등록 성공");

        reporter.report_progress(id, 30).expect("첫 보고");
        reporter.report_progress(id, 30).expect("같은 값 재보고");

        let progress_count = sink
            .events()
            .iter()
            .filter(|event| matches!(event, JobEvent::Progress { .. }))
            .count();

        assert_eq!(progress_count, 1);
    }

    #[test]
    fn 진행률은_상한으로_잘린_값이_이벤트에도_실린다() {
        let (reporter, sink) = reporter();
        let id = reporter.enqueue(request()).expect("등록 성공");

        reporter.report_progress(id, 200).expect("진행률 보고");

        assert!(sink
            .events()
            .contains(&JobEvent::Progress { id, progress: 100 }));
    }

    #[test]
    fn 모르는_id_는_에러이고_이벤트를_남기지_않는다() {
        let (reporter, sink) = reporter();

        let result = reporter.report_progress(999, 50);

        assert!(matches!(
            result,
            Err(ReportError::Job(JobError::NotFound(999)))
        ));
        assert!(sink.events().is_empty());
    }

    #[test]
    fn sink_발행_실패는_삼키지_않고_에러로_돌려준다() {
        let sink = Arc::new(RecordingSink::failing("웹뷰가 닫혔습니다"));
        let queue = Arc::new(JobQueue::new());
        let reporter = JobReporter::new(queue, Arc::clone(&sink));

        let result = reporter.enqueue(request());

        assert!(matches!(result, Err(ReportError::Emit(_))));
    }

    #[test]
    fn 실행_중_취소는_cancelling_후_cancelled_두_이벤트를_낸다() {
        let (reporter, sink) = reporter();
        let id = reporter.enqueue(request()).expect("등록 성공");
        reporter.claim_next().expect("워커가 작업을 집음");
        reporter.report_progress(id, 20).expect("실행 중 진행");

        let status = reporter.cancel(id).expect("취소 요청");
        assert_eq!(status, JobStatus::Cancelling);
        // 워커 루프는 이 질의로 중단 시점을 판단한다
        assert!(reporter.is_cancel_requested(id));

        reporter.mark_cancelled(id).expect("취소 확정");
        assert!(!reporter.is_cancel_requested(id));

        let events = sink.events();
        assert!(events.contains(&JobEvent::Cancelling { id }));
        assert!(events.contains(&JobEvent::Cancelled { id }));
    }

    #[test]
    fn 대기_중_취소는_cancelled_이벤트_하나로_끝난다() {
        let (reporter, sink) = reporter();
        let id = reporter.enqueue(request()).expect("등록 성공");

        let status = reporter.cancel(id).expect("취소");

        assert_eq!(status, JobStatus::Cancelled);
        assert!(sink.events().contains(&JobEvent::Cancelled { id }));
        assert!(!sink.events().contains(&JobEvent::Cancelling { id }));
    }

    // ── 프리플라이트 안내 ─────────────────────────────────────────

    #[test]
    fn 안내를_보고하면_note_이벤트가_발행된다() {
        // Arrange
        let (reporter, sink) = reporter();
        let id = reporter.enqueue(request()).expect("등록 성공");

        // Act
        reporter
            .note(id, "배포용(읽기 전용) 한글 문서입니다.")
            .expect("안내 보고");

        // Assert
        assert!(sink.events().contains(&JobEvent::Note {
            id,
            message: "배포용(읽기 전용) 한글 문서입니다.".to_string(),
        }));
    }

    #[test]
    fn 안내는_작업_상태나_진행률을_건드리지_않는다() {
        // Arrange — 실행 중이고 진행률이 올라간 작업
        let (reporter, _sink) = reporter();
        let id = reporter.enqueue(request()).expect("등록 성공");
        reporter.claim_next().expect("워커가 작업을 집음");
        reporter.report_progress(id, 5).expect("진행률 보고");

        // Act
        reporter.note(id, "안내").expect("안내 보고");

        // Assert — 안내는 순수 통지다. 상태를 바꾸면 UI 가 실패로 오해한다.
        let job = &reporter.snapshot()[0];
        assert_eq!(job.status, JobStatus::Running);
        assert_eq!(job.progress, 5);
    }

    #[test]
    fn 안내를_남긴_작업도_그대로_완료될_수_있다() {
        // Arrange
        let (reporter, sink) = reporter();
        let id = reporter.enqueue(request()).expect("등록 성공");
        reporter.claim_next().expect("워커가 작업을 집음");

        // Act
        reporter.note(id, "안내").expect("안내 보고");
        reporter.complete(id).expect("완료 처리");

        // Assert
        assert!(sink.events().contains(&JobEvent::Completed { id }));
        assert_eq!(reporter.snapshot()[0].status, JobStatus::Completed);
    }

    #[test]
    fn 모르는_id_에_안내를_보고하면_에러이고_이벤트를_남기지_않는다() {
        // Arrange
        let (reporter, sink) = reporter();

        // Act
        let result = reporter.note(999, "안내");

        // Assert
        assert!(matches!(
            result,
            Err(ReportError::Job(JobError::NotFound(999)))
        ));
        assert!(sink.events().is_empty());
    }

    #[test]
    fn note_이벤트도_kind_태그로_직렬화된다() {
        // Arrange & Act
        let json = serde_json::to_value(JobEvent::Note {
            id: 3,
            message: "안내".to_string(),
        })
        .expect("직렬화 성공");

        // Assert
        assert_eq!(
            json,
            serde_json::json!({ "kind": "note", "id": 3, "message": "안내" })
        );
    }

    #[test]
    fn 이벤트는_kind_태그가_붙은_json_으로_직렬화된다() {
        let json = serde_json::to_value(JobEvent::Progress {
            id: 7,
            progress: 42,
        })
        .expect("직렬화 성공");

        assert_eq!(
            json,
            serde_json::json!({ "kind": "progress", "id": 7, "progress": 42 })
        );
    }
}
