//! 프리플라이트 거부 사유를 사용자 대면 한국어 메시지로 옮긴다.
//!
//! 메시지는 "무엇이 문제인지"와 "다음에 무엇을 하면 되는지"를 함께 담는다.
//! 조치할 방법이 있는 사유(암호)는 반드시 해결 경로를 알려 준다.

use super::inspect::InspectError;
use super::preflight::RejectReason;

/// 문서를 검사조차 못 한 경우의 안내.
///
/// 원인 문자열(`cfb`·`zip` 크레이트의 영문 진단)은 절대 싣지 않는다 — 사용자에게
/// "DIFAT refers to sector 119" 를 보여 봤자 할 수 있는 일이 없다. 진단은 로그로 남긴다.
pub fn inspect_error_message(error: &InspectError) -> &'static str {
    match error {
        InspectError::Io(_) => {
            "파일을 열지 못했습니다. 파일이 옮겨졌거나 권한이 없는지 확인해 주세요."
        }
        InspectError::Malformed(_) => {
            "문서를 열지 못했습니다. 한글 문서가 아니거나 파일이 손상됐습니다."
        }
    }
}

/// 거부 사유에 대응하는 한국어 안내 문구.
pub fn reject_message(reason: RejectReason) -> &'static str {
    match reason {
        RejectReason::PasswordProtected => {
            "암호가 설정된 한글 문서입니다. 한글에서 암호를 해제한 뒤 다시 시도해 주세요."
        }
        RejectReason::DrmProtected => "DRM(보안)이 적용된 문서는 변환할 수 없습니다.",
        RejectReason::UnsupportedHwpV3 => {
            "HWP 3.0 이하 문서는 지원하지 않습니다. 한글에서 최신 형식으로 저장한 뒤 시도해 주세요."
        }
        RejectReason::NotHwpDocument => "한글 문서 형식이 아닙니다.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::hwp::inspect::InspectError;

    // ── 검사 실패 안내 ──

    #[test]
    fn 손상된_파일_안내에는_내부_진단이_새지_않는다() {
        // Arrange — cfb 크레이트가 뱉는 영문 진단이 그대로 사용자에게 갔었다.
        let error = InspectError::Malformed(
            "DIFAT refers to sector 119, but sector count is only 54".to_string(),
        );

        // Act
        let 메시지 = inspect_error_message(&error);

        // Assert
        for 내부어 in ["DIFAT", "sector", "CFB", "Invalid"] {
            assert!(!메시지.contains(내부어), "내부 진단이 샌다: {메시지}");
        }
        assert!(메시지.chars().any(|c| ('가'..='힣').contains(&c)));
        assert!(메시지.ends_with('.'));
    }

    #[test]
    fn 읽기_실패와_손상은_서로_다른_안내다() {
        // 파일을 못 연 것과 내용이 깨진 것은 사용자가 할 일이 다르다.
        let io = inspect_error_message(&InspectError::Io("permission denied".to_string()));
        let malformed = inspect_error_message(&InspectError::Malformed("boom".to_string()));

        assert_ne!(io, malformed);
        assert!(!io.contains("permission denied"), "원문이 샌다: {io}");
    }

    // ── happy path ──

    #[test]
    fn 암호_문서는_해제_방법을_알려_준다() {
        // Arrange & Act
        let 메시지 = reject_message(RejectReason::PasswordProtected);

        // Assert
        assert_eq!(
            메시지,
            "암호가 설정된 한글 문서입니다. 한글에서 암호를 해제한 뒤 다시 시도해 주세요."
        );
    }

    // ── edge cases ──

    #[test]
    fn 사유마다_서로_다른_메시지를_준다() {
        // Arrange
        let mut 메시지들: Vec<&str> = RejectReason::ALL
            .iter()
            .copied()
            .map(reject_message)
            .collect();

        // Act
        메시지들.sort_unstable();
        메시지들.dedup();

        // Assert
        assert_eq!(메시지들.len(), RejectReason::ALL.len());
    }

    #[test]
    fn 모든_거부_사유에_한국어_메시지가_있다() {
        // Arrange
        let 한글_범위 = |c: char| ('가'..='힣').contains(&c);

        for 사유 in RejectReason::ALL {
            // Act
            let 메시지 = reject_message(사유);

            // Assert
            assert!(!메시지.is_empty(), "{사유:?} 에 메시지가 없다");
            assert!(
                메시지.chars().any(한글_범위),
                "{사유:?} 메시지가 한국어가 아니다"
            );
            assert!(
                메시지.ends_with('.'),
                "{사유:?} 메시지가 문장으로 끝나지 않는다"
            );
        }
    }

    #[test]
    fn drm과_v3와_비한글문서_메시지가_각각_정확하다() {
        // Arrange & Act & Assert
        assert_eq!(
            reject_message(RejectReason::DrmProtected),
            "DRM(보안)이 적용된 문서는 변환할 수 없습니다."
        );
        assert_eq!(
            reject_message(RejectReason::UnsupportedHwpV3),
            "HWP 3.0 이하 문서는 지원하지 않습니다. 한글에서 최신 형식으로 저장한 뒤 시도해 주세요."
        );
        assert_eq!(
            reject_message(RejectReason::NotHwpDocument),
            "한글 문서 형식이 아닙니다."
        );
    }
}
