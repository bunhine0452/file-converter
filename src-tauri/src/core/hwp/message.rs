//! 프리플라이트 거부 사유를 사용자 대면 한국어 메시지로 옮긴다.
//!
//! 메시지는 "무엇이 문제인지"와 "다음에 무엇을 하면 되는지"를 함께 담는다.
//! 조치할 방법이 있는 사유(암호)는 반드시 해결 경로를 알려 준다.

use super::preflight::RejectReason;

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
