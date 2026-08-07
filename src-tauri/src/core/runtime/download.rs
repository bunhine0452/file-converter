//! 해시 검증 다운로더.
//!
//! 네트워크는 도구 바이너리를 받을 때만 쓴다 — 사용자 파일은 어떤 경우에도 나가지 않는다.
//! 받은 바이트는 스트리밍하면서 sha256 을 함께 계산하고, pin 한 해시와 다르면 즉시 버린다.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::core::runtime::assets::AssetSpec;

/// 진행 보고 최소 간격 — 이보다 촘촘하면 UI 이벤트가 폭주한다.
pub const PROGRESS_MIN_INTERVAL_MS: u64 = 200;
/// 진행 보고 최소 증가폭(퍼센트).
pub const PROGRESS_MIN_PERCENT_STEP: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub received: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    #[error("다운로드에 실패했습니다: {0}")]
    Network(String),
    #[error("파일 검증에 실패했습니다 (기대 {expected}, 실제 {actual})")]
    HashMismatch { expected: String, actual: String },
    #[error("다운로드를 취소했습니다")]
    Cancelled,
    #[error("파일을 저장하지 못했습니다: {0}")]
    Io(String),
}

pub trait Downloader: Send + Sync {
    /// 스트리밍하며 sha256 을 함께 계산한다. 불일치·취소 시 dest 를 삭제하고 Err.
    fn download_verified(
        &self,
        spec: &AssetSpec,
        dest: &Path,
        on_progress: &mut dyn FnMut(DownloadProgress),
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<(), DownloadError>;
}

/// 이미 받아둔 파일을 검증한다 (재시작 시 재다운로드 회피).
pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> Result<(), DownloadError> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);

    finish_verify(hasher, expected_hex)
}

/// 스트리밍하며 갱신한 해시를 마무리 비교한다.
fn finish_verify(hasher: Sha256, expected_hex: &str) -> Result<(), DownloadError> {
    let actual = hex_lower(&hasher.finalize());
    let expected = expected_hex.trim().to_ascii_lowercase();

    if actual == expected {
        return Ok(());
    }

    Err(DownloadError::HashMismatch { expected, actual })
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push_str(&format!("{byte:02x}"));
    }

    hex
}

/// 검증에 실패했거나 취소된 다운로드의 흔적을 지운다 — 부분 파일이 남으면
/// 다음 실행에서 "이미 받아둔 파일"로 오인된다.
fn discard(dest: &Path) {
    let _ = std::fs::remove_file(dest);
}

/// 진행 이벤트 폭주를 막는 스로틀.
///
/// `elapsed_ms` 는 다운로드 시작 이후 경과 시간이다 — 시계를 직접 읽지 않아 순수하게 테스트된다.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProgressThrottle {
    last: Option<Reported>,
}

#[derive(Debug, Clone, Copy)]
struct Reported {
    received: u64,
    at_ms: u64,
}

impl ProgressThrottle {
    pub fn new() -> Self {
        Self::default()
    }

    /// 마지막 보고 이후 1% 또는 200ms 이상 지났을 때만 true. 첫 이벤트와 완료는 항상 보고한다.
    pub fn should_report(&mut self, received: u64, total: Option<u64>, elapsed_ms: u64) -> bool {
        let report = match self.last {
            None => true,
            Some(last) => {
                is_complete(received, total)
                    || elapsed_ms.saturating_sub(last.at_ms) >= PROGRESS_MIN_INTERVAL_MS
                    || advanced_enough(received.saturating_sub(last.received), total)
            }
        };

        if report {
            self.last = Some(Reported {
                received,
                at_ms: elapsed_ms,
            });
        }

        report
    }
}

fn is_complete(received: u64, total: Option<u64>) -> bool {
    matches!(total, Some(total) if received >= total)
}

/// 총 크기를 모르면 퍼센트 규칙을 쓸 수 없다 — 시간 규칙에만 맡긴다.
fn advanced_enough(delta: u64, total: Option<u64>) -> bool {
    match total {
        Some(total) if total > 0 => delta.saturating_mul(100) >= total * PROGRESS_MIN_PERCENT_STEP,
        _ => false,
    }
}

/// 실제 다운로더.
///
/// 변환 워커 스레드에서 호출되므로 안에서 전용 런타임을 만들어 블로킹한다 —
/// 이 트레이트를 async 로 만들면 코어 전체가 async 로 물들고 테스트가 무거워진다.
#[derive(Debug, Default, Clone, Copy)]
pub struct ReqwestDownloader;

impl Downloader for ReqwestDownloader {
    fn download_verified(
        &self,
        spec: &AssetSpec,
        dest: &Path,
        on_progress: &mut dyn FnMut(DownloadProgress),
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<(), DownloadError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| DownloadError::Io(error.to_string()))?;

        let result = runtime.block_on(stream_to_file(spec, dest, on_progress, is_cancelled));

        // 실패한 다운로드의 부분 파일은 어떤 경우에도 남기지 않는다.
        if result.is_err() {
            discard(dest);
        }

        result
    }
}

async fn stream_to_file(
    spec: &AssetSpec,
    dest: &Path,
    on_progress: &mut dyn FnMut(DownloadProgress),
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), DownloadError> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let response = reqwest::get(spec.url)
        .await
        .map_err(|error| DownloadError::Network(error.to_string()))?;
    if !response.status().is_success() {
        return Err(DownloadError::Network(format!(
            "서버가 {} 를 돌려주었습니다",
            response.status()
        )));
    }

    let total = response.content_length();
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|error| DownloadError::Io(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        if is_cancelled() {
            return Err(DownloadError::Cancelled);
        }

        let chunk = chunk.map_err(|error| DownloadError::Network(error.to_string()))?;
        file.write_all(&chunk)
            .await
            .map_err(|error| DownloadError::Io(error.to_string()))?;
        hasher.update(&chunk);
        received += chunk.len() as u64;
        on_progress(DownloadProgress { received, total });
    }

    file.flush()
        .await
        .map_err(|error| DownloadError::Io(error.to_string()))?;
    drop(file);

    finish_verify(hasher, spec.sha256)
}

/// 테스트용 다운로더 — 미리 정한 바이트를 청크로 흘려보내며 실제와 같은 검증·정리를 거친다.
#[cfg(test)]
pub mod fake {
    use super::*;
    use std::io::Write;

    pub struct FakeDownloader {
        bytes: Vec<u8>,
        chunk_size: usize,
    }

    impl FakeDownloader {
        pub fn new(bytes: Vec<u8>) -> Self {
            let chunk_size = bytes.len().max(1);
            Self { bytes, chunk_size }
        }

        /// 취소·진행 보고를 여러 번 거치게 하려면 청크를 잘게 쪼갠다.
        pub fn chunk_size(mut self, chunk_size: usize) -> Self {
            self.chunk_size = chunk_size.max(1);
            self
        }
    }

    impl Downloader for FakeDownloader {
        fn download_verified(
            &self,
            spec: &AssetSpec,
            dest: &Path,
            on_progress: &mut dyn FnMut(DownloadProgress),
            is_cancelled: &dyn Fn() -> bool,
        ) -> Result<(), DownloadError> {
            let total = self.bytes.len() as u64;
            let mut file = std::fs::File::create(dest)
                .map_err(|error| DownloadError::Io(error.to_string()))?;
            let mut hasher = Sha256::new();
            let mut received = 0u64;

            for chunk in self.bytes.chunks(self.chunk_size) {
                if is_cancelled() {
                    drop(file);
                    discard(dest);
                    return Err(DownloadError::Cancelled);
                }

                file.write_all(chunk)
                    .map_err(|error| DownloadError::Io(error.to_string()))?;
                hasher.update(chunk);
                received += chunk.len() as u64;
                on_progress(DownloadProgress {
                    received,
                    total: Some(total),
                });
            }

            drop(file);

            if let Err(error) = finish_verify(hasher, spec.sha256) {
                discard(dest);
                return Err(error);
            }

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeDownloader;
    use super::*;
    use crate::core::runtime::assets::AssetSpec;
    use std::cell::Cell;

    /// `printf 'hello' | shasum -a 256`
    const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
    /// 어떤 내용과도 일치하지 않는 해시.
    const NEVER_MATCHING_SHA256: &str =
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

    fn spec(sha256: &'static str) -> AssetSpec {
        AssetSpec {
            url: "https://example.invalid/asset.bin",
            sha256,
        }
    }

    fn ignore_progress() -> impl FnMut(DownloadProgress) {
        |_| {}
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 해시가_일치하면_검증에_성공한다() {
        assert_eq!(verify_sha256(b"hello", HELLO_SHA256), Ok(()));
    }

    #[test]
    fn 다운로드한_바이트가_dest_에_그대로_쓰인다() {
        // Arrange
        let dir = tempfile::tempdir().expect("임시 디렉토리");
        let dest = dir.path().join("asset.bin");
        let downloader = FakeDownloader::new(b"hello".to_vec());
        let mut seen: Vec<DownloadProgress> = Vec::new();

        // Act
        let result = downloader.download_verified(
            &spec(HELLO_SHA256),
            &dest,
            &mut |progress| seen.push(progress),
            &|| false,
        );

        // Assert
        assert_eq!(result, Ok(()));
        assert_eq!(std::fs::read(&dest).expect("결과 파일"), b"hello");
        assert_eq!(seen.last().map(|p| p.received), Some(5));
        assert_eq!(seen.last().and_then(|p| p.total), Some(5));
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 기대_해시의_대소문자는_무시한다() {
        assert_eq!(
            verify_sha256(b"hello", &HELLO_SHA256.to_ascii_uppercase()),
            Ok(())
        );
    }

    #[test]
    fn 해시가_다르면_기대값과_실제값이_에러에_담긴다() {
        let expected = "0".repeat(64);

        let error = verify_sha256(b"hello", &expected).expect_err("불일치");

        assert_eq!(
            error,
            DownloadError::HashMismatch {
                expected,
                actual: HELLO_SHA256.to_string(),
            }
        );
    }

    #[test]
    fn 길이가_틀린_기대_해시도_불일치로_처리한다() {
        assert!(matches!(
            verify_sha256(b"hello", "deadbeef"),
            Err(DownloadError::HashMismatch { .. })
        ));
    }

    #[test]
    fn 해시가_다르면_받은_파일을_지운다() {
        let dir = tempfile::tempdir().expect("임시 디렉토리");
        let dest = dir.path().join("asset.bin");
        let downloader = FakeDownloader::new(b"hello".to_vec());

        let result = downloader.download_verified(
            &spec(NEVER_MATCHING_SHA256),
            &dest,
            &mut ignore_progress(),
            &|| false,
        );

        assert!(matches!(result, Err(DownloadError::HashMismatch { .. })));
        assert!(!dest.exists(), "검증 실패한 파일이 남으면 안 된다");
    }

    #[test]
    fn 취소하면_부분_파일이_남지_않는다() {
        // Arrange — 첫 청크를 쓴 뒤에 취소된다.
        let dir = tempfile::tempdir().expect("임시 디렉토리");
        let dest = dir.path().join("asset.bin");
        let downloader = FakeDownloader::new(b"hello".to_vec()).chunk_size(1);
        let checks = Cell::new(0usize);

        // Act
        let result = downloader.download_verified(
            &spec(HELLO_SHA256),
            &dest,
            &mut ignore_progress(),
            &|| {
                checks.set(checks.get() + 1);
                checks.get() > 1
            },
        );

        // Assert
        assert_eq!(result, Err(DownloadError::Cancelled));
        assert!(
            !dest.exists(),
            "취소된 다운로드의 부분 파일이 남으면 안 된다"
        );
    }

    #[test]
    fn 스로틀은_첫_이벤트를_보고하고_직후_잔변화는_거른다() {
        let mut throttle = ProgressThrottle::new();

        assert!(throttle.should_report(0, Some(1_000_000), 0));
        assert!(!throttle.should_report(10, Some(1_000_000), 10));
        assert!(!throttle.should_report(100, Some(1_000_000), 199));
    }

    #[test]
    fn 스로틀은_200ms_가_지나면_보고한다() {
        let mut throttle = ProgressThrottle::new();
        throttle.should_report(0, Some(1_000_000), 0);

        assert!(throttle.should_report(1, Some(1_000_000), 200));
        assert!(!throttle.should_report(2, Some(1_000_000), 300));
        assert!(throttle.should_report(3, Some(1_000_000), 400));
    }

    #[test]
    fn 스로틀은_1퍼센트_이상_진행하면_시간과_무관하게_보고한다() {
        let mut throttle = ProgressThrottle::new();
        throttle.should_report(0, Some(1_000), 0);

        assert!(!throttle.should_report(9, Some(1_000), 1));
        assert!(throttle.should_report(10, Some(1_000), 2));
    }

    #[test]
    fn 총_크기를_모르면_시간_규칙만_쓴다() {
        let mut throttle = ProgressThrottle::new();
        throttle.should_report(0, None, 0);

        assert!(!throttle.should_report(9_999_999, None, 100));
        assert!(throttle.should_report(10_000_000, None, 200));
    }

    #[test]
    fn 완료_이벤트는_항상_보고한다() {
        let mut throttle = ProgressThrottle::new();
        throttle.should_report(0, Some(100), 0);

        assert!(throttle.should_report(100, Some(100), 1));
    }

    #[test]
    fn 총_크기가_0_이어도_죽지_않는다() {
        let mut throttle = ProgressThrottle::new();

        assert!(throttle.should_report(0, Some(0), 0));
        assert!(throttle.should_report(0, Some(0), 1));
    }
}
