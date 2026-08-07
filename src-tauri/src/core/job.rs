//! 변환 작업 큐 — 등록·취소·상태 추적. 실제 변환 실행은 워커가 맡고, 여기서는 상태만 관리한다.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use crate::core::file_type::FileKind;

pub type JobId = u64;

/// 진행률 상한. UI 는 0~100 정수만 다룬다.
pub const PROGRESS_MAX: u8 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// 등록됐고 아직 워커가 가져가지 않음
    Queued,
    /// 워커가 실행 중
    Running,
    /// 실행 중 취소 요청을 받음 — 워커가 정리 후 `Cancelled` 로 확정한다
    Cancelling,
    Cancelled,
    Completed,
    Failed(String),
}

impl JobStatus {
    /// 더 이상 상태가 바뀌지 않는 종료 상태인가.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Cancelled | JobStatus::Completed | JobStatus::Failed(_)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRequest {
    pub source: PathBuf,
    pub target: FileKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: JobId,
    pub request: JobRequest,
    pub status: JobStatus,
    pub progress: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JobError {
    #[error("작업 {0}을(를) 찾을 수 없습니다")]
    NotFound(JobId),
    #[error("작업 {0}은(는) 이미 끝나 취소할 수 없습니다")]
    NotCancellable(JobId),
}

#[derive(Default)]
struct Inner {
    next_id: JobId,
    /// 등록 순서를 그대로 유지한다 (UI 큐 리스트가 이 순서를 쓴다).
    jobs: Vec<Job>,
}

/// 여러 스레드(워커·Tauri 커맨드)가 공유하는 큐.
pub struct JobQueue {
    inner: Mutex<Inner>,
}

impl JobQueue {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }

    /// 작업을 등록하고 발급된 id 를 돌려준다.
    pub fn enqueue(&self, request: JobRequest) -> JobId {
        let mut inner = self.lock();

        inner.next_id += 1;
        let id = inner.next_id;
        inner.jobs.push(Job {
            id,
            request,
            status: JobStatus::Queued,
            progress: 0,
        });

        id
    }

    /// 대기 중인 가장 오래된 작업을 가져와 `Running` 으로 전환한다.
    pub fn claim_next(&self) -> Option<Job> {
        let mut inner = self.lock();

        let job = inner
            .jobs
            .iter_mut()
            .find(|job| job.status == JobStatus::Queued)?;
        job.status = JobStatus::Running;

        Some(job.clone())
    }

    /// 취소를 요청한다. 대기 중이면 즉시 `Cancelled`, 실행 중이면 `Cancelling` 이 된다.
    pub fn cancel(&self, id: JobId) -> Result<JobStatus, JobError> {
        self.with_job(id, |job| match job.status {
            JobStatus::Queued => {
                job.status = JobStatus::Cancelled;
                Ok(JobStatus::Cancelled)
            }
            // 실행 중에는 즉시 죽일 수 없다 — 워커가 임시 파일을 정리한 뒤 확정한다.
            JobStatus::Running | JobStatus::Cancelling => {
                job.status = JobStatus::Cancelling;
                Ok(JobStatus::Cancelling)
            }
            _ => Err(JobError::NotCancellable(id)),
        })?
    }

    /// 워커가 취소 요청을 확인했는지 판단할 때 쓴다.
    pub fn is_cancel_requested(&self, id: JobId) -> bool {
        self.get(id)
            .is_some_and(|job| job.status == JobStatus::Cancelling)
    }

    pub fn set_progress(&self, id: JobId, progress: u8) -> Result<(), JobError> {
        self.with_job(id, |job| {
            job.progress = progress.min(PROGRESS_MAX);
        })
    }

    pub fn complete(&self, id: JobId) -> Result<(), JobError> {
        self.with_job(id, |job| {
            job.status = JobStatus::Completed;
            job.progress = PROGRESS_MAX;
        })
    }

    pub fn fail(&self, id: JobId, message: impl Into<String>) -> Result<(), JobError> {
        let message = message.into();
        self.with_job(id, |job| {
            job.status = JobStatus::Failed(message);
        })
    }

    /// 워커가 정리를 마치고 취소를 확정한다.
    pub fn mark_cancelled(&self, id: JobId) -> Result<(), JobError> {
        self.with_job(id, |job| {
            job.status = JobStatus::Cancelled;
        })
    }

    pub fn get(&self, id: JobId) -> Option<Job> {
        self.lock().jobs.iter().find(|job| job.id == id).cloned()
    }

    /// 등록 순서대로 전체 스냅샷을 돌려준다.
    pub fn list(&self) -> Vec<Job> {
        self.lock().jobs.clone()
    }

    fn with_job<T>(&self, id: JobId, apply: impl FnOnce(&mut Job) -> T) -> Result<T, JobError> {
        let mut inner = self.lock();
        let job = inner
            .jobs
            .iter_mut()
            .find(|job| job.id == id)
            .ok_or(JobError::NotFound(id))?;

        Ok(apply(job))
    }

    /// 잠금이 오염돼도 큐 상태(단순 Vec)는 그대로 쓸 수 있으므로 복구해서 계속 진행한다.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| {
            eprintln!("job queue mutex poisoned — 상태를 복구해 계속 진행합니다");
            poisoned.into_inner()
        })
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(name: &str) -> JobRequest {
        JobRequest {
            source: PathBuf::from(format!("/tmp/{name}")),
            target: FileKind::Pdf,
        }
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 등록한_작업은_대기_상태로_목록에_남는다() {
        let queue = JobQueue::new();

        let id = queue.enqueue(request("a.hwp"));

        let job = queue.get(id).expect("등록된 작업");
        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(job.progress, 0);
        assert_eq!(queue.list().len(), 1);
    }

    #[test]
    fn 서로_다른_작업은_서로_다른_id_를_받고_등록_순서를_유지한다() {
        let queue = JobQueue::new();

        let first = queue.enqueue(request("a.hwp"));
        let second = queue.enqueue(request("b.hwp"));

        assert_ne!(first, second);
        let ids: Vec<JobId> = queue.list().iter().map(|job| job.id).collect();
        assert_eq!(ids, vec![first, second]);
    }

    #[test]
    fn claim_next_는_가장_오래된_대기_작업을_실행_상태로_넘긴다() {
        let queue = JobQueue::new();
        let first = queue.enqueue(request("a.hwp"));
        let second = queue.enqueue(request("b.hwp"));

        let claimed = queue.claim_next().expect("가져올 작업");

        assert_eq!(claimed.id, first);
        assert_eq!(claimed.status, JobStatus::Running);
        assert_eq!(queue.get(second).unwrap().status, JobStatus::Queued);
    }

    #[test]
    fn 진행률_갱신과_완료가_상태에_반영된다() {
        let queue = JobQueue::new();
        let id = queue.enqueue(request("a.hwp"));
        queue.claim_next();

        queue.set_progress(id, 42).expect("진행률 갱신");
        assert_eq!(queue.get(id).unwrap().progress, 42);

        queue.complete(id).expect("완료 처리");
        let job = queue.get(id).unwrap();
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.progress, PROGRESS_MAX);
    }

    #[test]
    fn 실패는_사유_메시지를_보존한다() {
        let queue = JobQueue::new();
        let id = queue.enqueue(request("a.hwp"));
        queue.claim_next();

        queue.fail(id, "암호가 걸린 문서입니다").expect("실패 처리");

        assert_eq!(
            queue.get(id).unwrap().status,
            JobStatus::Failed("암호가 걸린 문서입니다".to_string())
        );
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 대기_중_취소는_즉시_확정되고_워커가_가져가지_않는다() {
        let queue = JobQueue::new();
        let cancelled = queue.enqueue(request("a.hwp"));
        let alive = queue.enqueue(request("b.hwp"));

        let status = queue.cancel(cancelled).expect("취소 성공");

        assert_eq!(status, JobStatus::Cancelled);
        assert_eq!(queue.claim_next().map(|job| job.id), Some(alive));
    }

    #[test]
    fn 실행_중_취소는_요청_상태를_거쳐_워커가_확정한다() {
        let queue = JobQueue::new();
        let id = queue.enqueue(request("a.hwp"));
        queue.claim_next();

        let status = queue.cancel(id).expect("취소 요청 성공");

        assert_eq!(status, JobStatus::Cancelling);
        assert!(queue.is_cancel_requested(id));

        queue.mark_cancelled(id).expect("취소 확정");
        assert_eq!(queue.get(id).unwrap().status, JobStatus::Cancelled);
        assert!(!queue.is_cancel_requested(id));
    }

    #[test]
    fn 이미_끝난_작업은_취소할_수_없다() {
        let queue = JobQueue::new();
        let id = queue.enqueue(request("a.hwp"));
        queue.claim_next();
        queue.complete(id).expect("완료 처리");

        assert_eq!(queue.cancel(id), Err(JobError::NotCancellable(id)));
    }

    #[test]
    fn 모르는_id_는_조회도_조작도_실패한다() {
        let queue = JobQueue::new();
        let missing: JobId = 999;

        assert!(queue.get(missing).is_none());
        assert_eq!(queue.cancel(missing), Err(JobError::NotFound(missing)));
        assert_eq!(
            queue.set_progress(missing, 10),
            Err(JobError::NotFound(missing))
        );
        assert!(!queue.is_cancel_requested(missing));
    }

    #[test]
    fn 진행률은_상한을_넘지_않도록_잘린다() {
        let queue = JobQueue::new();
        let id = queue.enqueue(request("a.hwp"));
        queue.claim_next();

        queue.set_progress(id, 250).expect("진행률 갱신");

        assert_eq!(queue.get(id).unwrap().progress, PROGRESS_MAX);
    }

    #[test]
    fn 대기_작업이_없으면_claim_next_는_none_이다() {
        let queue = JobQueue::new();

        assert!(queue.claim_next().is_none());
    }

    #[test]
    fn 취소된_작업의_진행률_갱신은_상태를_되돌리지_않는다() {
        let queue = JobQueue::new();
        let id = queue.enqueue(request("a.hwp"));
        queue.cancel(id).expect("취소 성공");

        queue.set_progress(id, 80).expect("진행률 갱신 자체는 성공");

        assert_eq!(queue.get(id).unwrap().status, JobStatus::Cancelled);
    }

    #[test]
    fn 큐는_여러_스레드에서_동시에_등록해도_모두_기록한다() {
        use std::sync::Arc;

        let queue = Arc::new(JobQueue::new());
        let handles: Vec<_> = (0..8)
            .map(|index| {
                let queue = Arc::clone(&queue);
                std::thread::spawn(move || queue.enqueue(request(&format!("{index}.hwp"))))
            })
            .collect();

        let ids: Vec<JobId> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(queue.list().len(), 8);
        let unique: std::collections::HashSet<JobId> = ids.into_iter().collect();
        assert_eq!(unique.len(), 8);
    }
}
