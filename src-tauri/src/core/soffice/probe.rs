//! soffice 탐지에 필요한 환경 조회 포트.
//!
//! 환경변수·홈 디렉토리·PATH·Windows 레지스트리·파일 존재 확인을 한 트레이트로 묶어
//! 탐지 로직이 실제 머신 상태와 무관하게 단위 테스트되도록 한다.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Hive {
    LocalMachine,
    CurrentUser,
}

/// Windows 레지스트리 리다이렉션 뷰. 32비트 LibreOffice 가 아직 배포되므로 둘 다 본다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegView {
    Bits64,
    Bits32,
}

pub trait SofficeProbe: Send + Sync {
    fn env_var(&self, key: &str) -> Option<String>;
    fn home_dir(&self) -> Option<PathBuf>;
    /// 앱이 직접 설치·관리하는 런타임 루트 (없으면 None).
    fn managed_root(&self) -> Option<PathBuf>;
    /// 사용자가 설정에서 직접 지정한 soffice 경로 (최우선 후보).
    fn user_override(&self) -> Option<PathBuf>;
    fn is_executable_file(&self, path: &Path) -> bool;
    /// `name` 이 빈 문자열이면 (Default) 값. 비-Windows 에서는 항상 None.
    fn registry_string(
        &self,
        hive: Hive,
        subkey: &str,
        name: &str,
        view: RegView,
    ) -> Option<String>;
    /// PATH 에서 실행 파일을 찾는다.
    fn find_in_path(&self, name: &str) -> Option<PathBuf>;
}

/// 실제 환경.
#[derive(Debug, Clone, Default)]
pub struct RealProbe {
    managed_root: Option<PathBuf>,
    user_override: Option<PathBuf>,
}

impl RealProbe {
    pub fn new(managed_root: Option<PathBuf>, user_override: Option<PathBuf>) -> Self {
        Self {
            managed_root,
            user_override,
        }
    }
}

impl SofficeProbe for RealProbe {
    fn env_var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn home_dir(&self) -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }

    fn managed_root(&self) -> Option<PathBuf> {
        self.managed_root.clone()
    }

    fn user_override(&self) -> Option<PathBuf> {
        self.user_override.clone()
    }

    fn is_executable_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    #[cfg(windows)]
    fn registry_string(
        &self,
        hive: Hive,
        subkey: &str,
        name: &str,
        view: RegView,
    ) -> Option<String> {
        use winreg::enums::{
            HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
        };
        use winreg::RegKey;

        let root = match hive {
            Hive::LocalMachine => RegKey::predef(HKEY_LOCAL_MACHINE),
            Hive::CurrentUser => RegKey::predef(HKEY_CURRENT_USER),
        };
        let flags = KEY_READ
            | match view {
                RegView::Bits64 => KEY_WOW64_64KEY,
                RegView::Bits32 => KEY_WOW64_32KEY,
            };

        root.open_subkey_with_flags(subkey, flags)
            .ok()?
            .get_value::<String, _>(name)
            .ok()
    }

    #[cfg(not(windows))]
    fn registry_string(
        &self,
        _hive: Hive,
        _subkey: &str,
        _name: &str,
        _view: RegView,
    ) -> Option<String> {
        None
    }

    fn find_in_path(&self, name: &str) -> Option<PathBuf> {
        which::which(name).ok()
    }
}

/// 테스트용 환경 — 원하는 파일·레지스트리·PATH 만 존재하는 세계를 만든다.
#[cfg(test)]
pub mod fake {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Debug, Default, Clone)]
    pub struct FakeProbe {
        env: BTreeMap<String, String>,
        home: Option<PathBuf>,
        managed_root: Option<PathBuf>,
        user_override: Option<PathBuf>,
        executables: BTreeSet<PathBuf>,
        registry: BTreeMap<(Hive, String, String, RegView), String>,
        path_entries: BTreeMap<String, PathBuf>,
    }

    impl FakeProbe {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn env(mut self, key: &str, value: &str) -> Self {
            self.env.insert(key.to_string(), value.to_string());
            self
        }

        pub fn home(mut self, path: impl Into<PathBuf>) -> Self {
            self.home = Some(path.into());
            self
        }

        pub fn managed_root(mut self, path: impl Into<PathBuf>) -> Self {
            self.managed_root = Some(path.into());
            self
        }

        pub fn user_override(mut self, path: impl Into<PathBuf>) -> Self {
            self.user_override = Some(path.into());
            self
        }

        pub fn executable(mut self, path: impl Into<PathBuf>) -> Self {
            self.executables.insert(path.into());
            self
        }

        pub fn registry(
            mut self,
            hive: Hive,
            subkey: &str,
            name: &str,
            view: RegView,
            value: &str,
        ) -> Self {
            self.registry.insert(
                (hive, subkey.to_string(), name.to_string(), view),
                value.to_string(),
            );
            self
        }

        pub fn on_path(mut self, name: &str, path: impl Into<PathBuf>) -> Self {
            self.path_entries.insert(name.to_string(), path.into());
            self
        }
    }

    impl SofficeProbe for FakeProbe {
        fn env_var(&self, key: &str) -> Option<String> {
            self.env.get(key).cloned()
        }

        fn home_dir(&self) -> Option<PathBuf> {
            self.home.clone()
        }

        fn managed_root(&self) -> Option<PathBuf> {
            self.managed_root.clone()
        }

        fn user_override(&self) -> Option<PathBuf> {
            self.user_override.clone()
        }

        fn is_executable_file(&self, path: &Path) -> bool {
            self.executables.contains(path)
        }

        fn registry_string(
            &self,
            hive: Hive,
            subkey: &str,
            name: &str,
            view: RegView,
        ) -> Option<String> {
            self.registry
                .get(&(hive, subkey.to_string(), name.to_string(), view))
                .cloned()
        }

        fn find_in_path(&self, name: &str) -> Option<PathBuf> {
            self.path_entries.get(name).cloned()
        }
    }
}
