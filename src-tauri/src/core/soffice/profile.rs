//! soffice 전용 사용자 프로필 URL.
//!
//! 같은 `UserInstallation` 으로 soffice 를 병렬 실행하면 한쪽이 경고 없이 산출물 없이 끝난다.
//! 그래서 모든 호출에 전용 프로필을 붙이고, 프로필 경로는 `file://` URL 로 넘겨야 한다.

use std::ffi::OsString;
use std::path::Path;

/// `-env:UserInstallation=` 에 붙는 값. 하이픈은 하나다 (`--env` 아님).
pub const USER_INSTALLATION_PREFIX: &str = "-env:UserInstallation=";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileUrl(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProfileUrlError {
    #[error("프로필 경로는 절대 경로여야 합니다: {0}")]
    NotAbsolute(String),
}

impl ProfileUrl {
    /// 호스트 OS 규칙으로 절대 경로를 `file://` URL 로 바꾼다.
    pub fn from_dir(path: &Path) -> Result<Self, ProfileUrlError> {
        Self::from_path_str(&path.to_string_lossy(), cfg!(windows))
    }

    /// 플랫폼을 인자로 받는 순수 버전 — 어느 호스트에서도 양쪽 규칙을 테스트할 수 있다.
    pub fn from_path_str(path: &str, windows_style: bool) -> Result<Self, ProfileUrlError> {
        if !is_absolute(path, windows_style) {
            return Err(ProfileUrlError::NotAbsolute(path.to_string()));
        }

        let trimmed = trim_trailing_separators(path, windows_style);
        let normalized = if windows_style {
            trimmed.replace('\\', "/")
        } else {
            trimmed.to_string()
        };

        Ok(ProfileUrl(format!(
            "file://{}",
            authority_and_path(&normalized, windows_style)
        )))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// soffice 에 그대로 넘길 인자 하나.
    pub fn as_arg(&self) -> OsString {
        OsString::from(format!("{USER_INSTALLATION_PREFIX}{}", self.0))
    }
}

fn is_absolute(path: &str, windows_style: bool) -> bool {
    if !windows_style {
        return path.starts_with('/');
    }

    is_unc(path) || drive_prefix_len(path) > 0
}

fn is_unc(path: &str) -> bool {
    path.starts_with("\\\\") || path.starts_with("//")
}

/// `C:\` / `C:/` 형태면 2, 아니면 0.
fn drive_prefix_len(path: &str) -> usize {
    let bytes = path.as_bytes();
    let looks_like_drive = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/');

    if looks_like_drive {
        2
    } else {
        0
    }
}

fn trim_trailing_separators(path: &str, windows_style: bool) -> &str {
    let trimmed = path.trim_end_matches('/');
    let trimmed = if windows_style {
        trimmed.trim_end_matches('\\')
    } else {
        trimmed
    };

    // 루트만 남았다면 원본을 유지한다 (`/` → `/`).
    if trimmed.is_empty() {
        path
    } else {
        trimmed
    }
}

/// `file://` 뒤에 붙을 부분. UNC 는 슬래시 두 개를 그대로 두고, 드라이브는 앞에 하나를 더한다.
fn authority_and_path(normalized: &str, windows_style: bool) -> String {
    if !windows_style {
        return percent_encode_path(normalized);
    }

    if let Some(rest) = normalized.strip_prefix("//") {
        return format!("//{}", percent_encode_path(rest));
    }

    let drive_len = drive_prefix_len(normalized);
    if drive_len > 0 {
        let (drive, rest) = normalized.split_at(drive_len);
        return format!("/{drive}{}", percent_encode_path(rest));
    }

    percent_encode_path(normalized)
}

/// RFC 3986 unreserved + `/` 만 남기고 나머지 바이트는 퍼센트 인코딩한다.
fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());

    for byte in path.as_bytes() {
        let byte = *byte;
        let keep = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/');
        if keep {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unix(path: &str) -> ProfileUrl {
        ProfileUrl::from_path_str(path, false).expect("유닉스 절대 경로")
    }

    fn windows(path: &str) -> ProfileUrl {
        ProfileUrl::from_path_str(path, true).expect("윈도 절대 경로")
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 유닉스_절대경로는_file_url_로_바뀐다() {
        assert_eq!(unix("/tmp/fc-profile").as_str(), "file:///tmp/fc-profile");
    }

    #[test]
    fn 인자는_하이픈_하나짜리_env_접두사를_쓴다() {
        let arg = unix("/tmp/p").as_arg();

        assert_eq!(arg, OsString::from("-env:UserInstallation=file:///tmp/p"));
        assert!(!arg.to_string_lossy().starts_with("--env"));
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 공백은_퍼센트_인코딩된다() {
        assert_eq!(
            unix("/Users/kim/Library/Application Support/fc").as_str(),
            "file:///Users/kim/Library/Application%20Support/fc"
        );
    }

    #[test]
    fn 한글_경로는_utf8_퍼센트_인코딩된다() {
        // "가" = EA B0 80
        assert_eq!(unix("/tmp/가").as_str(), "file:///tmp/%EA%B0%80");
    }

    #[test]
    fn 윈도_경로는_드라이브_문자를_살리고_역슬래시를_슬래시로_바꾼다() {
        assert_eq!(
            windows(r"C:\Users\kim\AppData\Local\fc").as_str(),
            "file:///C:/Users/kim/AppData/Local/fc"
        );
    }

    #[test]
    fn 윈도_경로의_공백도_인코딩된다() {
        assert_eq!(
            windows(r"C:\Program Files\fc").as_str(),
            "file:///C:/Program%20Files/fc"
        );
    }

    #[test]
    fn 안전한_문자는_인코딩하지_않는다() {
        assert_eq!(unix("/tmp/a-b_c.d~e/f").as_str(), "file:///tmp/a-b_c.d~e/f");
    }

    #[test]
    fn 끝의_구분자는_붙지_않는다() {
        // soffice 는 트레일링 슬래시가 붙은 프로필 URL 에서 오작동한 보고가 있다.
        assert_eq!(unix("/tmp/p/").as_str(), "file:///tmp/p");
        assert_eq!(windows(r"C:\p\").as_str(), "file:///C:/p");
    }

    #[test]
    fn 상대경로는_거부한다() {
        assert!(matches!(
            ProfileUrl::from_path_str("tmp/p", false),
            Err(ProfileUrlError::NotAbsolute(_))
        ));
        assert!(matches!(
            ProfileUrl::from_path_str(r"Users\p", true),
            Err(ProfileUrlError::NotAbsolute(_))
        ));
    }

    #[test]
    fn 윈도_unc_경로도_절대경로로_받아준다() {
        assert_eq!(
            windows(r"\\server\share\fc").as_str(),
            "file:////server/share/fc"
        );
    }
}
