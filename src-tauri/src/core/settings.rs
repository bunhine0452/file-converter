//! 사용자 설정 — 저장 위치·이름 규칙·테마.
//!
//! 설정 파일이 깨졌다고 앱이 뜨지 않으면 사용자는 복구할 방법이 없다. 그래서 읽기는
//! **절대 실패하지 않는다** — 못 읽으면 기본값으로 시작하고, 다음 저장에서 정상 파일이 된다.

use serde::{Deserialize, Serialize};

/// 변환 결과를 어디에 둘지.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SaveMode {
    /// 변환할 때마다 저장 위치를 묻는다.
    #[default]
    Ask,
    /// 원본이 있던 폴더에 그대로 둔다.
    SameAsSource,
    /// 정해 둔 폴더에 모은다.
    FixedFolder,
}

/// 같은 이름이 이미 있을 때.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ConflictRule {
    /// `보고서 (1).pdf` 처럼 번호를 붙인다.
    #[default]
    Number,
    /// 기존 파일을 덮어쓴다.
    Overwrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    /// OS 설정을 따른다.
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub save_mode: SaveMode,
    /// `FixedFolder` 일 때 쓸 폴더. 고르지 않았으면 None.
    pub output_dir: Option<String>,
    /// 파일명 끝(확장자 앞)에 붙일 말. 비어 있으면 원본 이름 그대로.
    pub name_suffix: String,
    pub on_conflict: ConflictRule,
    pub theme: Theme,
}

/// 설정 JSON 을 읽는다. 깨졌거나 비었으면 기본값 — 절대 실패하지 않는다.
pub fn parse_settings(json: &str) -> Settings {
    serde_json::from_str(json).unwrap_or_default()
}

/// 저장용 JSON. 프론트가 그대로 쓰므로 camelCase 다.
pub fn to_json(settings: &Settings) -> String {
    serde_json::to_string_pretty(settings).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 기본값은_매번_묻고_번호를_붙이고_시스템_테마다() {
        let settings = Settings::default();

        assert_eq!(settings.save_mode, SaveMode::Ask);
        assert_eq!(settings.on_conflict, ConflictRule::Number);
        assert_eq!(settings.theme, Theme::System);
        assert!(settings.name_suffix.is_empty());
        assert_eq!(settings.output_dir, None);
    }

    #[test]
    fn 저장했다_읽으면_값이_그대로다() {
        let settings = Settings {
            save_mode: SaveMode::FixedFolder,
            output_dir: Some("/Users/kim/변환결과".to_string()),
            name_suffix: "_변환".to_string(),
            on_conflict: ConflictRule::Overwrite,
            theme: Theme::Dark,
        };

        assert_eq!(parse_settings(&to_json(&settings)), settings);
    }

    #[test]
    fn json_은_프론트가_쓰는_camel_case_다() {
        let json = to_json(&Settings {
            save_mode: SaveMode::SameAsSource,
            ..Settings::default()
        });

        assert!(json.contains("\"saveMode\": \"sameAsSource\""), "{json}");
        assert!(json.contains("\"onConflict\""), "{json}");
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 깨진_설정_파일은_기본값으로_되돌린다() {
        // 설정이 깨졌다고 앱이 안 뜨면 사용자는 복구할 방법이 없다.
        for broken in ["", "{", "null", "[]", "그냥 텍스트"] {
            assert_eq!(parse_settings(broken), Settings::default(), "{broken:?}");
        }
    }

    #[test]
    fn 일부_필드만_있으면_나머지는_기본값이다() {
        let settings = parse_settings(r#"{"theme":"dark"}"#);

        assert_eq!(settings.theme, Theme::Dark);
        assert_eq!(settings.save_mode, SaveMode::Ask);
    }

    #[test]
    fn 모르는_필드가_있어도_읽는다() {
        // 옛 버전이 남긴 키 하나 때문에 설정을 통째로 잃으면 안 된다.
        let settings = parse_settings(r#"{"theme":"light","무언가":42}"#);

        assert_eq!(settings.theme, Theme::Light);
    }

    #[test]
    fn 모르는_값은_기본값으로_떨어진다() {
        // 신버전이 쓴 모드를 구버전이 읽는 경우.
        let settings = parse_settings(r#"{"saveMode":"미래모드"}"#);

        assert_eq!(settings.save_mode, SaveMode::Ask);
    }
}
