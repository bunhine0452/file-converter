//! 변환 성공/실패 판정.
//!
//! soffice 는 실패해도 exit 0 을 내는 일이 잦고(tdf#148275), 반대로 시그널로 죽어도
//! PDF 는 멀쩡히 남는 경우가 있다. 그래서 종료 코드 하나로 판정하지 않고
//! **타임아웃 → stderr 패턴 → 종료 상태 → 산출물** 순으로 증거를 겹쳐 본다.
//!
//! 입력을 [`JudgeInput`] 구조체로 받아 파일 시스템을 모르는 순수 함수로 유지한다.

use super::runner::Termination;

/// PDF 파일의 선두 5바이트.
pub const PDF_MAGIC: [u8; 5] = *b"%PDF-";

/// 이보다 작은 PDF 는 본문이 비었을 가능성이 높다 (암호 문서·빈 문서 사례).
pub const MIN_PLAUSIBLE_PDF_BYTES: u64 = 1024;

/// 기동 중 UNO 예외로 죽었을 때 soffice 가 내는 종료 코드.
const STARTUP_FATAL_EXIT: i32 = 77;

/// stderr 문자열만으로 크래시를 알았을 때처럼 시그널 번호를 모르는 경우.
const SIGNAL_UNKNOWN: i32 = 0;

const JAVA_PARSE_MARKERS: &[&str] = &["HwpDoc.Exception.HwpParseException", "at HwpDoc.HwpFile."];
const CRASH_MARKER: &str = "Unspecified Application Error";
const SOURCE_NOT_LOADED_MARKER: &str = "Error: source file could not be loaded";
const NO_EXPORT_FILTER_MARKER: &str = "Error: no export filter";
const OTHER_MARKERS: &[&str] = &[
    "Error: Please verify input parameters",
    "Error: Cannot create temporary file",
];

/// 판정에 필요한 증거 묶음. 산출물은 이미 읽어 온 값으로 넘긴다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeInput<'a> {
    pub termination: Termination,
    pub stdout: &'a str,
    pub stderr: &'a str,
    pub output_exists: bool,
    pub output_len: u64,
    /// 산출물 선두 5바이트. 5바이트를 못 읽었으면 `None`.
    pub output_magic: Option<[u8; 5]>,
    /// LibreOffice 26.2 이상이면 true — 그때만 종료 코드를 근거로 쓴다.
    pub trusts_exit_code: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertOutcome {
    Ok,
    /// PDF 는 만들어졌지만 내용이 비었을 수 있다 — 사용자에게 확인을 권한다.
    SuspectEmpty {
        reason: String,
    },
    Failed(ConvertFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertFailure {
    /// 확장 미등록·JRE 없음·감지 실패 — 대개 런타임 구성 문제다.
    SourceNotLoaded,
    /// H2Orestart 가 문서를 해석하다 던진 Java 예외.
    JavaParseError,
    Crashed {
        signal: i32,
    },
    StartupFatal,
    TimedOut,
    NoExportFilter,
    OutputMissing,
    OutputNotPdf,
    Other(String),
}

/// 증거를 우선순위대로 훑어 결과를 정한다.
pub fn judge(input: &JudgeInput<'_>) -> ConvertOutcome {
    if input.termination == Termination::TimedOut {
        return ConvertOutcome::Failed(ConvertFailure::TimedOut);
    }

    // stderr 의 확정 패턴은 exit 0 이어도 실패다.
    if let Some(failure) = failure_from_stderr(input.stderr) {
        return ConvertOutcome::Failed(failure);
    }

    if let Some(failure) = failure_from_termination(input) {
        return ConvertOutcome::Failed(failure);
    }

    judge_output(input)
}

fn failure_from_stderr(stderr: &str) -> Option<ConvertFailure> {
    // 스택이 찍혔다면 확장은 이미 로드된 것이므로 로드 실패보다 먼저 본다.
    if JAVA_PARSE_MARKERS.iter().any(|m| stderr.contains(m)) {
        return Some(ConvertFailure::JavaParseError);
    }
    if stderr.contains(CRASH_MARKER) {
        return Some(ConvertFailure::Crashed {
            signal: SIGNAL_UNKNOWN,
        });
    }
    if stderr.contains(SOURCE_NOT_LOADED_MARKER) {
        return Some(ConvertFailure::SourceNotLoaded);
    }
    if stderr.contains(NO_EXPORT_FILTER_MARKER) {
        return Some(ConvertFailure::NoExportFilter);
    }

    OTHER_MARKERS
        .iter()
        .find(|marker| stderr.contains(**marker))
        .map(|marker| ConvertFailure::Other((*marker).to_string()))
}

fn failure_from_termination(input: &JudgeInput<'_>) -> Option<ConvertFailure> {
    match input.termination {
        // 시그널은 OS 가 알려주는 사실이라 버전과 무관하다. 다만 종료 단계에서
        // 죽는 사례가 있어 산출물이 멀쩡하면 크래시로 치지 않는다.
        Termination::Signal(signal) if !has_valid_pdf(input) => {
            Some(ConvertFailure::Crashed { signal })
        }
        Termination::Signal(_) => None,
        // 77 은 tdf#148275 이전부터 있던 기동 실패 규약이라 버전과 무관하다.
        Termination::Code(STARTUP_FATAL_EXIT) => Some(ConvertFailure::StartupFatal),
        // 그 외 코드는 26.2 미만에서 신뢰할 수 없다 — 실패해도 exit 0 을 내기 때문이다.
        Termination::Code(_) if !input.trusts_exit_code => None,
        Termination::Code(0) => None,
        Termination::Code(code) => Some(ConvertFailure::Other(format!(
            "soffice 가 종료 코드 {code} 로 끝났습니다"
        ))),
        Termination::TimedOut => Some(ConvertFailure::TimedOut),
    }
}

fn judge_output(input: &JudgeInput<'_>) -> ConvertOutcome {
    if !input.output_exists {
        return ConvertOutcome::Failed(ConvertFailure::OutputMissing);
    }
    if input.output_magic != Some(PDF_MAGIC) {
        return ConvertOutcome::Failed(ConvertFailure::OutputNotPdf);
    }
    if input.output_len < MIN_PLAUSIBLE_PDF_BYTES {
        return ConvertOutcome::SuspectEmpty {
            reason: format!(
                "PDF 크기가 {}바이트뿐이라 내용이 비었을 수 있습니다",
                input.output_len
            ),
        };
    }

    ConvertOutcome::Ok
}

fn has_valid_pdf(input: &JudgeInput<'_>) -> bool {
    input.output_exists && input.output_magic == Some(PDF_MAGIC)
}

/// 사용자에게 보여 줄 한국어 안내. 다음 행동을 알려 주는 문장으로 쓴다.
pub fn failure_message(failure: &ConvertFailure) -> String {
    match failure {
        ConvertFailure::SourceNotLoaded => {
            "HWP 지원 구성요소가 준비되지 않았습니다. 설정에서 다시 설치해 주세요.".to_string()
        }
        ConvertFailure::JavaParseError => {
            "문서 일부를 해석하지 못했습니다. HWPX(.hwpx)로 다시 저장한 뒤 시도해 보세요."
                .to_string()
        }
        ConvertFailure::Crashed { .. } => {
            "문서 구조를 해석하는 중 오류가 발생했습니다.".to_string()
        }
        ConvertFailure::StartupFatal => {
            "변환 엔진을 시작하지 못했습니다. 잠시 후 다시 시도해 주세요.".to_string()
        }
        ConvertFailure::TimedOut => "변환이 제한 시간을 초과했습니다.".to_string(),
        ConvertFailure::NoExportFilter => "PDF 내보내기 필터를 찾지 못했습니다.".to_string(),
        ConvertFailure::OutputMissing | ConvertFailure::OutputNotPdf => {
            "변환 결과 파일이 만들어지지 않았습니다.".to_string()
        }
        ConvertFailure::Other(_) => "변환에 실패했습니다. 다시 시도해 주세요.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PDF_BYTES: u64 = 42_000;

    /// 성공 상황을 기본값으로 두고 테스트마다 필요한 필드만 바꾼다.
    fn judged(input: JudgeInput<'_>) -> ConvertOutcome {
        judge(&input)
    }

    fn success<'a>() -> JudgeInput<'a> {
        JudgeInput {
            termination: Termination::Code(0),
            stdout: "convert /in/a.hwp as a Writer document -> /out/a.pdf \
                     using filter : writer_pdf_Export",
            stderr: "",
            output_exists: true,
            output_len: PDF_BYTES,
            output_magic: Some(*b"%PDF-"),
            trusts_exit_code: true,
        }
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 정상_종료와_정상_pdf_는_성공이다() {
        // Arrange
        let input = success();

        // Act
        let outcome = judged(input);

        // Assert
        assert_eq!(outcome, ConvertOutcome::Ok);
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 타임아웃은_다른_모든_단서보다_먼저_판정된다() {
        let outcome = judged(JudgeInput {
            termination: Termination::TimedOut,
            // 산출물이 남아 있어도 타임아웃이 우선이다.
            ..success()
        });

        assert_eq!(outcome, ConvertOutcome::Failed(ConvertFailure::TimedOut));
    }

    #[test]
    fn 신뢰_버전에서_exit_1_은_실패다() {
        let outcome = judged(JudgeInput {
            termination: Termination::Code(1),
            trusts_exit_code: true,
            ..success()
        });

        assert!(matches!(outcome, ConvertOutcome::Failed(_)));
    }

    #[test]
    fn 비신뢰_버전에서는_exit_1_을_무시하고_산출물로_판정한다() {
        // 26.2 미만은 실패해도 exit 0 을 내므로 종료 코드를 근거로 쓸 수 없다.
        let ok = judged(JudgeInput {
            termination: Termination::Code(1),
            trusts_exit_code: false,
            ..success()
        });
        let missing = judged(JudgeInput {
            termination: Termination::Code(1),
            trusts_exit_code: false,
            output_exists: false,
            output_len: 0,
            output_magic: None,
            ..success()
        });

        assert_eq!(ok, ConvertOutcome::Ok);
        assert_eq!(
            missing,
            ConvertOutcome::Failed(ConvertFailure::OutputMissing)
        );
    }

    #[test]
    fn source_could_not_be_loaded_는_exit_0_이어도_실패다() {
        let outcome = judged(JudgeInput {
            stderr: "Error: source file could not be loaded",
            ..success()
        });

        assert_eq!(
            outcome,
            ConvertOutcome::Failed(ConvertFailure::SourceNotLoaded)
        );
    }

    #[test]
    fn hwp_파싱_예외는_자바_파싱_실패로_분류한다() {
        let outcome = judged(JudgeInput {
            stderr: "HwpDoc.Exception.HwpParseException: unknown tag",
            ..success()
        });

        assert_eq!(
            outcome,
            ConvertOutcome::Failed(ConvertFailure::JavaParseError)
        );
    }

    #[test]
    fn 자바_스택이_함께_있으면_로드_실패보다_파싱_실패를_우선한다() {
        // 스택이 찍혔다는 건 확장이 이미 로드됐다는 뜻이라 재설치 안내는 틀린 답이다.
        let outcome = judged(JudgeInput {
            stderr: "at HwpDoc.HwpFile.open(HwpFile.java:120)\n\
                     Error: source file could not be loaded",
            ..success()
        });

        assert_eq!(
            outcome,
            ConvertOutcome::Failed(ConvertFailure::JavaParseError)
        );
    }

    #[test]
    fn no_export_filter_는_전용_실패로_분류한다() {
        let outcome = judged(JudgeInput {
            stderr: "Error: no export filter for /out/a.pdf",
            ..success()
        });

        assert_eq!(
            outcome,
            ConvertOutcome::Failed(ConvertFailure::NoExportFilter)
        );
    }

    #[test]
    fn 임시파일_생성_실패는_other_로_분류한다() {
        let outcome = judged(JudgeInput {
            stderr: "Error: Cannot create temporary file",
            ..success()
        });

        match outcome {
            ConvertOutcome::Failed(ConvertFailure::Other(detail)) => {
                assert!(detail.contains("Cannot create temporary file"));
            }
            other => panic!("Other 로 분류되어야 한다: {other:?}"),
        }
    }

    #[test]
    fn unspecified_application_error_는_크래시_계열이다() {
        let outcome = judged(JudgeInput {
            stderr: "Unspecified Application Error",
            ..success()
        });

        assert!(matches!(
            outcome,
            ConvertOutcome::Failed(ConvertFailure::Crashed { .. })
        ));
    }

    #[test]
    fn sigabrt_는_크래시로_분류한다() {
        let outcome = judged(JudgeInput {
            termination: Termination::Signal(134),
            output_exists: false,
            output_len: 0,
            output_magic: None,
            ..success()
        });

        assert_eq!(
            outcome,
            ConvertOutcome::Failed(ConvertFailure::Crashed { signal: 134 })
        );
    }

    #[test]
    fn sigsegv_라도_정상_pdf_면_성공이다() {
        // 종료 단계에서 죽는 사례가 있어 산출물이 멀쩡하면 통과시킨다.
        let outcome = judged(JudgeInput {
            termination: Termination::Signal(139),
            ..success()
        });

        assert_eq!(outcome, ConvertOutcome::Ok);
    }

    #[test]
    fn 비신뢰_버전이어도_시그널_종료는_크래시로_본다() {
        // 시그널은 OS 가 알려주는 사실이라 tdf#148275 의 영향을 받지 않는다.
        let outcome = judged(JudgeInput {
            termination: Termination::Signal(134),
            trusts_exit_code: false,
            output_exists: false,
            output_len: 0,
            output_magic: None,
            ..success()
        });

        assert_eq!(
            outcome,
            ConvertOutcome::Failed(ConvertFailure::Crashed { signal: 134 })
        );
    }

    #[test]
    fn exit_77_은_기동_실패다() {
        let outcome = judged(JudgeInput {
            termination: Termination::Code(77),
            output_exists: false,
            output_len: 0,
            output_magic: None,
            ..success()
        });

        assert_eq!(
            outcome,
            ConvertOutcome::Failed(ConvertFailure::StartupFatal)
        );
    }

    #[test]
    fn exit_77_은_구버전에서도_기동_실패로_본다() {
        // 77(EXITHELPER_FATAL_ERROR)은 tdf#148275 이전부터 있던 규약이라
        // "26.2 미만은 exit code 를 믿지 않는다" 규칙의 예외다.
        let outcome = judged(JudgeInput {
            termination: Termination::Code(77),
            trusts_exit_code: false,
            output_exists: false,
            output_len: 0,
            output_magic: None,
            ..success()
        });

        assert_eq!(
            outcome,
            ConvertOutcome::Failed(ConvertFailure::StartupFatal)
        );
    }

    #[test]
    fn 산출물이_없으면_output_missing_이다() {
        let outcome = judged(JudgeInput {
            output_exists: false,
            output_len: 0,
            output_magic: None,
            ..success()
        });

        assert_eq!(
            outcome,
            ConvertOutcome::Failed(ConvertFailure::OutputMissing)
        );
    }

    #[test]
    fn 매직이_pdf_가_아니면_output_not_pdf_이다() {
        let outcome = judged(JudgeInput {
            output_magic: Some(*b"PK\x03\x04\x00"),
            ..success()
        });

        assert_eq!(
            outcome,
            ConvertOutcome::Failed(ConvertFailure::OutputNotPdf)
        );
    }

    #[test]
    fn 매직을_읽지_못한_산출물도_output_not_pdf_이다() {
        let outcome = judged(JudgeInput {
            output_magic: None,
            ..success()
        });

        assert_eq!(
            outcome,
            ConvertOutcome::Failed(ConvertFailure::OutputNotPdf)
        );
    }

    #[test]
    fn 너무_작은_산출물은_의심으로_표시한다() {
        let outcome = judged(JudgeInput {
            output_len: 300,
            ..success()
        });

        match outcome {
            ConvertOutcome::SuspectEmpty { reason } => assert!(reason.contains("300")),
            other => panic!("SuspectEmpty 여야 한다: {other:?}"),
        }
    }

    #[test]
    fn 모든_실패에_한국어_메시지가_있다() {
        let all = [
            ConvertFailure::SourceNotLoaded,
            ConvertFailure::JavaParseError,
            ConvertFailure::Crashed { signal: 134 },
            ConvertFailure::StartupFatal,
            ConvertFailure::TimedOut,
            ConvertFailure::NoExportFilter,
            ConvertFailure::OutputMissing,
            ConvertFailure::OutputNotPdf,
            ConvertFailure::Other("boom".to_string()),
        ];

        for failure in &all {
            let message = failure_message(failure);
            assert!(!message.trim().is_empty(), "{failure:?} 메시지가 비었다");
            assert!(
                message.chars().any(|c| ('가'..='힣').contains(&c)),
                "{failure:?} 메시지가 한국어가 아니다: {message}"
            );
        }

        // 변형이 늘면 이 match 가 깨져서 메시지 누락을 컴파일 타임에 잡는다.
        for failure in &all {
            match failure {
                ConvertFailure::SourceNotLoaded
                | ConvertFailure::JavaParseError
                | ConvertFailure::Crashed { .. }
                | ConvertFailure::StartupFatal
                | ConvertFailure::TimedOut
                | ConvertFailure::NoExportFilter
                | ConvertFailure::OutputMissing
                | ConvertFailure::OutputNotPdf
                | ConvertFailure::Other(_) => {}
            }
        }
    }

    #[test]
    fn 안내_메시지는_실패마다_구분된다() {
        assert_ne!(
            failure_message(&ConvertFailure::SourceNotLoaded),
            failure_message(&ConvertFailure::JavaParseError)
        );
        assert_eq!(
            failure_message(&ConvertFailure::OutputMissing),
            failure_message(&ConvertFailure::OutputNotPdf)
        );
    }
}
