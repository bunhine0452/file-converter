//! 설치 계획 수립과 `unopkg` 출력 파싱.
//!
//! 순서가 곧 계약이다: JRE 가 준비되기 전에 soffice 프로필이 초기화되면
//! 이후 `JAVA_HOME` 을 올바로 줘도 계속 "source file could not be loaded" 로 실패하고,
//! 복구 수단은 프로필 삭제뿐이다. 그래서 이 계획은 항상 JRE 를 확장 설치보다 앞에 둔다.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::core::runtime::assets::{
    h2orestart_asset, jre_asset, libreoffice_asset, AssetSpec, Os, Platform, H2O_IDENTIFIER,
    H2O_VERSION,
};
use crate::core::soffice::profile::ProfileUrl;
use crate::core::soffice::version::LoVersion;

/// 앱이 관리하는 런타임을 담는 하위 디렉토리 이름.
pub const RUNTIME_DIR_NAME: &str = "runtime";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledLibreOffice {
    pub version: LoVersion,
    /// 앱이 직접 설치한 것인가 (아니면 사용자가 설치한 시스템 LibreOffice).
    pub managed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionState {
    Registered {
        version: String,
    },
    NotRegistered,
    /// `unopkg` 출력을 해석하지 못했다 — 준비됐다고 단정하면 안 된다.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStatus {
    pub libreoffice: Option<InstalledLibreOffice>,
    pub java_home: Option<PathBuf>,
    pub extension: ExtensionState,
    /// JRE 없이 초기화돼 Java 를 영영 못 찾는 프로필인가.
    pub profile_poisoned: bool,
}

/// 확장을 어디에 설치할 것인가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionStrategy {
    /// 앱이 설치한 LibreOffice — 번들 확장 디렉토리에 직접 푼다.
    BundledDir,
    /// 사용자가 설치한 시스템 LibreOffice — 설치 디렉토리를 건드리지 않고 전용 프로필에 넣는다.
    UserProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallStep {
    DownloadLibreOffice(AssetSpec),
    InstallLibreOffice,
    DownloadJre(AssetSpec),
    InstallJre,
    DownloadExtension(AssetSpec),
    InstallExtension(ExtensionStrategy),
    /// 오염된 프로필을 지운다 — 확장을 다시 깔기 전에 반드시 선행해야 한다.
    ResetProfile,
    VerifyExtension,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InstallRootError {
    #[error("로밍 프로필에는 런타임을 설치할 수 없습니다: {0}")]
    RoamingNotAllowed(String),
}

/// 지금 상태에서 무엇을 어떤 순서로 해야 하는지. 이미 준비된 것은 건너뛴다(멱등).
pub fn resolve_install_plan(status: &RuntimeStatus, platform: Platform) -> Vec<InstallStep> {
    let mut steps = Vec::new();

    let needs_libreoffice = status.libreoffice.is_none();
    if needs_libreoffice {
        steps.push(InstallStep::DownloadLibreOffice(libreoffice_asset(
            platform,
        )));
        steps.push(InstallStep::InstallLibreOffice);
    }

    let needs_jre = status.java_home.is_none();
    if needs_jre {
        steps.push(InstallStep::DownloadJre(jre_asset(platform)));
        steps.push(InstallStep::InstallJre);
    }

    if status.profile_poisoned {
        steps.push(InstallStep::ResetProfile);
    }

    // LibreOffice·JRE 를 새로 깔았거나 프로필을 지웠으면 확장 등록도 함께 사라진다.
    let needs_extension = needs_libreoffice
        || needs_jre
        || status.profile_poisoned
        || extension_needs_install(&status.extension);

    if needs_extension {
        steps.push(InstallStep::DownloadExtension(h2orestart_asset()));
        steps.push(InstallStep::InstallExtension(extension_strategy(status)));
        steps.push(InstallStep::VerifyExtension);
    }

    steps
}

fn extension_needs_install(state: &ExtensionState) -> bool {
    match state {
        ExtensionState::Registered { version } => version != H2O_VERSION,
        ExtensionState::NotRegistered | ExtensionState::Unknown => true,
    }
}

/// LibreOffice 가 아직 없으면 우리가 설치할 것이므로 번들 전략이 된다.
fn extension_strategy(status: &RuntimeStatus) -> ExtensionStrategy {
    let managed = status
        .libreoffice
        .as_ref()
        .map(|installed| installed.managed)
        .unwrap_or(true);

    extension_strategy_for(managed)
}

/// 확장을 어디에 넣고 어디서 찾을지는 같은 질문이다 — 설치와 조회가 갈라지지 않게 한 곳에서 정한다.
pub fn extension_strategy_for(managed: bool) -> ExtensionStrategy {
    if managed {
        ExtensionStrategy::BundledDir
    } else {
        ExtensionStrategy::UserProfile
    }
}

/// `unopkg list` 출력에서 우리 확장의 등록 상태만 뽑는다.
///
/// 한 확장 블록에는 `is registered:` 가 여러 번 나온다 — 첫 줄이 확장 자체이고,
/// `bundled Packages` 안의 나머지는 하위 패키지다. **하나라도 미등록이면 준비된 것이 아니다**:
/// 확장 자체가 `yes` 여도 `H2Orestart.jar` 이 꺼져 있으면 HWP 필터는 동작하지 않는다.
pub fn parse_unopkg_list(stdout: &str) -> ExtensionState {
    // 출력을 "이해했는가" 와 "우리 확장이 있는가" 는 다른 질문이다.
    // 확장이 하나도 없다는 사실을 읽어낸 것과, 명령이 실패해 아무것도 모르는 것을 구분한다.
    let mut understood = stdout.contains("<none>");
    let mut current_identifier: Option<&str> = None;
    let mut current_version: Option<String> = None;
    let mut own: Option<(bool, Option<String>)> = None;
    let mut children_registered = true;

    for line in stdout.lines() {
        let line = line.trim();

        if let Some(identifier) = line.strip_prefix("Identifier:") {
            understood = true;
            current_identifier = Some(identifier.trim());
            current_version = None;
        } else if let Some(version) = line.strip_prefix("Version:") {
            current_version = Some(version.trim().to_string());
        } else if let Some(registered) = line.strip_prefix("is registered:") {
            if current_identifier != Some(H2O_IDENTIFIER) {
                continue;
            }

            // `unknown` 은 설치가 중간에 깨진 상태다 — 등록으로 볼 수 없다.
            let is_registered = registered.trim().eq_ignore_ascii_case("yes");
            match own {
                None => own = Some((is_registered, current_version.clone())),
                Some(_) => children_registered = children_registered && is_registered,
            }
        }
    }

    match own {
        Some((true, version)) if children_registered => ExtensionState::Registered {
            version: version.unwrap_or_default(),
        },
        Some(_) => ExtensionState::NotRegistered,
        None if understood => ExtensionState::NotRegistered,
        None => ExtensionState::Unknown,
    }
}

/// `unopkg add` argv. `--shared` 는 관리자 권한을 요구하므로 절대 쓰지 않는다.
pub fn unopkg_add_args(oxt: &Path, profile: &ProfileUrl) -> Vec<OsString> {
    vec![
        OsString::from("add"),
        // 헤드리스에서 라이선스 동의 프롬프트가 뜨면 그대로 멈춰버린다.
        OsString::from("--suppress-license"),
        profile.as_arg(),
        oxt.as_os_str().to_os_string(),
    ]
}

/// 프로필에 남은 잠금 파일 때문에 unopkg 가 시작조차 못 한 경우인가.
///
/// 비정상 종료(강제 종료·크래시)로 `.lock` 이 남으면 이후 모든 호출이 막힌다.
/// 우리는 전용 프로필을 쓰므로 이 잠금은 항상 우리가 남긴 찌꺼기다.
pub fn is_stale_lock_error(stderr: &str) -> bool {
    stderr.contains("The lock file indicates it is already running")
}

/// 지울 잠금 파일 경로.
pub fn profile_lock_file(profile_dir: &Path) -> PathBuf {
    profile_dir.join(".lock")
}

/// `unopkg list` argv. **스코프를 설치 전략과 맞춰야 한다** — 옵션 없는 `list` 는
/// 사용자 확장만 나열하므로 번들 디렉토리에 넣은 확장은 영영 보이지 않는다.
pub fn unopkg_list_args(profile: &ProfileUrl, strategy: ExtensionStrategy) -> Vec<OsString> {
    match strategy {
        ExtensionStrategy::BundledDir => vec![
            OsString::from("list"),
            OsString::from("--bundled"),
            profile.as_arg(),
        ],
        ExtensionStrategy::UserProfile => vec![OsString::from("list"), profile.as_arg()],
    }
}

/// 런타임을 풀어 놓을 디렉토리. Windows 로밍 프로필은 거부한다 —
/// 수백 MB 짜리 런타임이 로밍되면 기업 환경의 로그인이 망가진다.
pub fn managed_install_root(
    platform: Platform,
    app_data_dir: &Path,
) -> Result<PathBuf, InstallRootError> {
    if platform.os == Os::Windows && is_roaming(app_data_dir) {
        return Err(InstallRootError::RoamingNotAllowed(
            app_data_dir.display().to_string(),
        ));
    }

    Ok(app_data_dir.join(RUNTIME_DIR_NAME))
}

/// 경로 구분자가 호스트와 다를 수 있으므로(개발 머신에서 Windows 경로를 검사한다)
/// 컴포넌트가 아니라 문자열로 판단한다.
fn is_roaming(path: &Path) -> bool {
    let lowered = path.to_string_lossy().to_ascii_lowercase();

    lowered.contains("appdata\\roaming") || lowered.contains("appdata/roaming")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::runtime::assets::{Arch, Os};

    const MAC: Platform = Platform {
        os: Os::MacOs,
        arch: Arch::Aarch64,
    };
    const WIN: Platform = Platform {
        os: Os::Windows,
        arch: Arch::X86_64,
    };

    /// 모든 것이 준비된 기본 상태 — 각 테스트는 여기서 한 가지만 무너뜨린다.
    fn ready() -> RuntimeStatus {
        RuntimeStatus {
            libreoffice: Some(InstalledLibreOffice {
                version: LoVersion::new(26, 2, 5, 2),
                managed: true,
            }),
            java_home: Some(PathBuf::from("/opt/jre")),
            extension: ExtensionState::Registered {
                version: H2O_VERSION.to_string(),
            },
            profile_poisoned: false,
        }
    }

    fn index_of(steps: &[InstallStep], wanted: &InstallStep) -> usize {
        steps
            .iter()
            .position(|step| step == wanted)
            .unwrap_or_else(|| panic!("계획에 {wanted:?} 가 없다: {steps:?}"))
    }

    fn extension_install_index(steps: &[InstallStep]) -> usize {
        steps
            .iter()
            .position(|step| matches!(step, InstallStep::InstallExtension(_)))
            .expect("확장 설치 단계")
    }

    fn profile() -> ProfileUrl {
        ProfileUrl::from_path_str("/tmp/fc-profile", false).expect("프로필 URL")
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 전부_준비되면_할_일이_없다() {
        assert_eq!(resolve_install_plan(&ready(), MAC), Vec::new());
    }

    #[test]
    fn 아무것도_없으면_다운로드부터_검증까지_순서대로_계획한다() {
        // Arrange
        let status = RuntimeStatus {
            libreoffice: None,
            java_home: None,
            extension: ExtensionState::NotRegistered,
            profile_poisoned: false,
        };

        // Act
        let steps = resolve_install_plan(&status, MAC);

        // Assert
        assert_eq!(
            steps,
            vec![
                InstallStep::DownloadLibreOffice(libreoffice_asset(MAC)),
                InstallStep::InstallLibreOffice,
                InstallStep::DownloadJre(jre_asset(MAC)),
                InstallStep::InstallJre,
                InstallStep::DownloadExtension(h2orestart_asset()),
                InstallStep::InstallExtension(ExtensionStrategy::BundledDir),
                InstallStep::VerifyExtension,
            ]
        );
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 시스템_libreoffice_가_감지되면_다운로드하지_않는다() {
        let status = RuntimeStatus {
            libreoffice: Some(InstalledLibreOffice {
                version: LoVersion::new(26, 2, 5, 2),
                managed: false,
            }),
            extension: ExtensionState::NotRegistered,
            ..ready()
        };

        let steps = resolve_install_plan(&status, MAC);

        assert!(!steps
            .iter()
            .any(|step| matches!(step, InstallStep::DownloadLibreOffice(_))));
        assert!(!steps.contains(&InstallStep::InstallLibreOffice));
    }

    #[test]
    fn jre_설치는_반드시_확장_설치보다_앞선다() {
        // JRE 없이 프로필이 초기화되면 이후 JAVA_HOME 을 줘도 계속 실패한다.
        let status = RuntimeStatus {
            java_home: None,
            extension: ExtensionState::NotRegistered,
            ..ready()
        };

        let steps = resolve_install_plan(&status, MAC);

        assert!(index_of(&steps, &InstallStep::InstallJre) < extension_install_index(&steps));
        assert!(
            index_of(&steps, &InstallStep::InstallJre)
                < index_of(&steps, &InstallStep::VerifyExtension)
        );
    }

    #[test]
    fn 오염된_프로필은_초기화_단계를_거쳐_확장을_다시_깐다() {
        let status = RuntimeStatus {
            profile_poisoned: true,
            ..ready()
        };

        let steps = resolve_install_plan(&status, MAC);

        assert!(index_of(&steps, &InstallStep::ResetProfile) < extension_install_index(&steps));
    }

    #[test]
    fn 확장_버전이_드리프트하면_다시_설치한다() {
        let status = RuntimeStatus {
            extension: ExtensionState::Registered {
                version: "0.7.9".to_string(),
            },
            ..ready()
        };

        let steps = resolve_install_plan(&status, MAC);

        assert!(steps.contains(&InstallStep::DownloadExtension(h2orestart_asset())));
        assert!(steps.contains(&InstallStep::VerifyExtension));
    }

    #[test]
    fn 확장_상태를_모르면_설치를_다시_시도한다() {
        let status = RuntimeStatus {
            extension: ExtensionState::Unknown,
            ..ready()
        };

        assert!(!resolve_install_plan(&status, MAC).is_empty());
    }

    #[test]
    fn 앱이_설치한_libreoffice_는_번들_디렉토리에_확장을_넣는다() {
        let status = RuntimeStatus {
            extension: ExtensionState::NotRegistered,
            ..ready()
        };

        let steps = resolve_install_plan(&status, MAC);

        assert!(steps.contains(&InstallStep::InstallExtension(
            ExtensionStrategy::BundledDir
        )));
    }

    #[test]
    fn 시스템_설치본에는_사용자_프로필로_확장을_넣는다() {
        let status = RuntimeStatus {
            libreoffice: Some(InstalledLibreOffice {
                version: LoVersion::new(26, 2, 5, 2),
                managed: false,
            }),
            extension: ExtensionState::NotRegistered,
            ..ready()
        };

        let steps = resolve_install_plan(&status, MAC);

        assert!(steps.contains(&InstallStep::InstallExtension(
            ExtensionStrategy::UserProfile
        )));
    }

    #[test]
    fn 플랫폼에_맞는_자산을_계획에_담는다() {
        let status = RuntimeStatus {
            libreoffice: None,
            java_home: None,
            ..ready()
        };

        let steps = resolve_install_plan(&status, WIN);

        assert!(steps.contains(&InstallStep::DownloadLibreOffice(libreoffice_asset(WIN))));
        assert!(steps.contains(&InstallStep::DownloadJre(jre_asset(WIN))));
    }

    #[test]
    fn unopkg_목록에서_등록된_우리_확장을_읽는다() {
        let stdout = "All deployed user extensions:\n\n\
             Identifier: ebandal.libreoffice.H2Orestart\n   \
             Version: 0.7.13\n   \
             URL: file:///tmp/H2Orestart.oxt\n   \
             is registered: yes\n";

        assert_eq!(
            parse_unopkg_list(stdout),
            ExtensionState::Registered {
                version: "0.7.13".to_string()
            }
        );
    }

    #[test]
    fn 등록되지_않은_확장은_not_registered_다() {
        let stdout = "Identifier: ebandal.libreoffice.H2Orestart\n   \
             Version: 0.7.13\n   \
             is registered: no\n";

        assert_eq!(parse_unopkg_list(stdout), ExtensionState::NotRegistered);
    }

    #[test]
    fn 확장이_하나도_없으면_none_출력을_읽는다() {
        assert_eq!(
            parse_unopkg_list("All deployed user extensions:\n<none>\n"),
            ExtensionState::NotRegistered
        );
    }

    #[test]
    fn 다른_확장이_섞여_있어도_우리_것만_본다() {
        let stdout = "All deployed user extensions:\n\n\
             Identifier: org.other.Extension\n   \
             Version: 9.9.9\n   \
             is registered: yes\n\n\
             Identifier: ebandal.libreoffice.H2Orestart\n   \
             Version: 0.7.13\n   \
             is registered: yes\n\n\
             Identifier: com.another.Thing\n   \
             Version: 1.0\n   \
             is registered: no\n";

        assert_eq!(
            parse_unopkg_list(stdout),
            ExtensionState::Registered {
                version: "0.7.13".to_string()
            }
        );
    }

    #[test]
    fn 다른_확장만_있으면_우리_것은_미등록이다() {
        let stdout = "Identifier: org.other.Extension\n   Version: 9.9.9\n   is registered: yes\n";

        assert_eq!(parse_unopkg_list(stdout), ExtensionState::NotRegistered);
    }

    #[test]
    fn 해석할_수_없는_출력은_unknown_이다() {
        assert_eq!(parse_unopkg_list(""), ExtensionState::Unknown);
        assert_eq!(
            parse_unopkg_list("ERROR: unopkg failed"),
            ExtensionState::Unknown
        );
    }

    #[test]
    fn unopkg_add_argv_에_shared_가_없고_라이선스_억제가_있다() {
        let oxt = PathBuf::from("/tmp/H2Orestart.oxt");

        let args = unopkg_add_args(&oxt, &profile());

        let rendered: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(rendered[0], "add");
        assert!(
            !rendered.iter().any(|a| a == "--shared"),
            "--shared 는 관리자 권한을 요구한다: {rendered:?}"
        );
        assert!(rendered.iter().any(|a| a == "--suppress-license"));
        assert!(rendered.iter().any(|a| a == "/tmp/H2Orestart.oxt"));
        assert!(rendered
            .iter()
            .any(|a| a == "-env:UserInstallation=file:///tmp/fc-profile"));
    }

    #[test]
    fn unopkg_list_argv_는_프로필을_붙인다() {
        let args = unopkg_list_args(&profile(), ExtensionStrategy::UserProfile);

        let rendered: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(rendered[0], "list");
        assert!(rendered
            .iter()
            .any(|a| a == "-env:UserInstallation=file:///tmp/fc-profile"));
        assert!(!rendered.iter().any(|a| a == "--shared"));
    }

    #[test]
    fn 번들_디렉토리에_넣은_확장은_번들_스코프로_조회한다() {
        // `unopkg list` 는 사용자 확장만 나열한다 — 번들 확장은 `--bundled` 없이는
        // 영영 보이지 않아 설치가 성공해도 검증이 실패하고 매번 다시 설치한다.
        let args = unopkg_list_args(&profile(), ExtensionStrategy::BundledDir);

        let rendered: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(rendered[0], "list");
        assert!(rendered.iter().any(|a| a == "--bundled"), "{rendered:?}");
    }

    #[test]
    fn 사용자_프로필_확장은_번들_스코프로_조회하지_않는다() {
        let args = unopkg_list_args(&profile(), ExtensionStrategy::UserProfile);

        assert!(!args.iter().any(|a| a == "--bundled"));
    }

    #[test]
    fn 하위_패키지가_하나라도_미등록이면_등록으로_보지_않는다() {
        // 실제 출력 형태: 확장 자체는 yes 인데 Java 컴포넌트만 활성화에 실패한 상태.
        // 이때 HWP 필터는 동작하지 않으므로 준비됨으로 보고하면 안 된다.
        let stdout = "Identifier: ebandal.libreoffice.H2Orestart\n  \
             Version: 0.7.13\n  \
             URL: vnd.sun.star.expand:$BUNDLED_EXTENSIONS/H2Orestart\n  \
             is registered: yes\n  \
             bundled Packages: {\n      \
             URL: .../types.rdb\n      \
             is registered: yes\n\n      \
             URL: .../H2Orestart.jar\n      \
             is registered: no\n\n      \
             URL: .../registry/TypeDetection.xcu\n      \
             is registered: yes\n  }\n";

        assert_eq!(parse_unopkg_list(stdout), ExtensionState::NotRegistered);
    }

    #[test]
    fn 하위_패키지가_모두_등록되면_확장도_등록이다() {
        let stdout = "Identifier: ebandal.libreoffice.H2Orestart\n  \
             Version: 0.7.13\n  \
             is registered: yes\n  \
             bundled Packages: {\n      \
             URL: .../H2Orestart.jar\n      \
             is registered: yes\n\n      \
             URL: .../registry/TypeDetection.xcu\n      \
             is registered: yes\n  }\n";

        assert_eq!(
            parse_unopkg_list(stdout),
            ExtensionState::Registered {
                version: "0.7.13".to_string()
            }
        );
    }

    #[test]
    fn 등록_여부가_unknown_이면_등록으로_보지_않는다() {
        // 설치가 중간에 깨지면 unopkg 가 확장 상태를 unknown 으로 보고한다.
        let stdout = "Identifier: ebandal.libreoffice.H2Orestart\n  \
             Version: 0.7.13\n  \
             is registered: unknown\n";

        assert_eq!(parse_unopkg_list(stdout), ExtensionState::NotRegistered);
    }

    /// macOS LibreOffice 26.2.5.2 + H2Orestart 0.7.13 에서 그대로 캡처한 출력.
    /// 설명문에 URL·한글이 섞여 있어도 파서가 흔들리지 않아야 한다.
    const REAL_BUNDLED_LIST: &str = "All bundled extensions:\n\n\
         Identifier: ebandal.libreoffice.H2Orestart\n  \
         Version: 0.7.13\n  \
         URL: vnd.sun.star.expand:$BUNDLED_EXTENSIONS/H2Orestart\n  \
         is registered: yes\n  \
         Media-Type: application/vnd.sun.star.package-bundle\n  \
         Description: LibreOffice HWP 5.0 import Extension.\n\
         This product was developed by referring to the ᄒᆞᆫ글 document file (HWP, HWPML) \
         published by 한글과컴퓨터.\n\
         Please report bugs to https://github.com/ebandal/H2Orestart\n  \
         bundled Packages: {\n      \
         URL: vnd.sun.star.expand:$BUNDLED_EXTENSIONS/H2Orestart/types.rdb\n      \
         is registered: yes\n      \
         Media-Type: application/vnd.sun.star.uno-typelibrary;type=RDB\n      \
         Description: \n\n      \
         URL: vnd.sun.star.expand:$BUNDLED_EXTENSIONS/H2Orestart/H2Orestart.jar\n      \
         is registered: yes\n      \
         Media-Type: application/vnd.sun.star.uno-component;type=Java\n      \
         Description: \n\n      \
         URL: vnd.sun.star.expand:$BUNDLED_EXTENSIONS/H2Orestart/registry/TypeDetection.xcu\n      \
         is registered: yes\n      \
         Media-Type: application/vnd.sun.star.configuration-data\n      \
         Description: \n\n  }\n";

    #[test]
    fn 실제_번들_목록_출력을_등록으로_읽는다() {
        assert_eq!(
            parse_unopkg_list(REAL_BUNDLED_LIST),
            ExtensionState::Registered {
                version: "0.7.13".to_string()
            }
        );
    }

    #[test]
    fn 뒤에_오는_다른_확장의_하위_패키지는_우리_판정에_섞이지_않는다() {
        let stdout = "Identifier: ebandal.libreoffice.H2Orestart\n  \
             Version: 0.7.13\n  \
             is registered: yes\n\n\
             Identifier: org.other.Extension\n  \
             Version: 9.9.9\n  \
             is registered: no\n  \
             bundled Packages: {\n      \
             is registered: no\n  }\n";

        assert_eq!(
            parse_unopkg_list(stdout),
            ExtensionState::Registered {
                version: "0.7.13".to_string()
            }
        );
    }

    #[test]
    fn 잠금_파일_때문에_시작하지_못한_경우를_알아본다() {
        let stderr = "ERROR: unopkg cannot be started. The lock file indicates it is already \
             running. If this does not apply, delete the lock file at:\n/tmp/profile/.lock";

        assert!(is_stale_lock_error(stderr));
    }

    #[test]
    fn 다른_실패는_잠금_문제로_보지_않는다() {
        assert!(!is_stale_lock_error(""));
        assert!(!is_stale_lock_error(
            "ERROR: Exception occurred: NoConnectException"
        ));
        assert!(!is_stale_lock_error("lock"));
    }

    #[test]
    fn 잠금_파일은_프로필_디렉토리_바로_아래에_있다() {
        assert_eq!(
            profile_lock_file(Path::new("/data/profile")),
            PathBuf::from("/data/profile/.lock")
        );
    }

    #[test]
    fn windows_설치_목적지는_localappdata_계열이어야_한다() {
        let local = Path::new(r"C:\Users\kim\AppData\Local\file-converter");

        let root = managed_install_root(WIN, local).expect("LocalAppData 는 허용");

        assert!(root.starts_with(local));
        assert!(root.ends_with(RUNTIME_DIR_NAME));
    }

    #[test]
    fn windows_로밍_프로필은_설치_목적지로_거부한다() {
        let roaming = Path::new(r"C:\Users\kim\AppData\Roaming\file-converter");

        assert!(matches!(
            managed_install_root(WIN, roaming),
            Err(InstallRootError::RoamingNotAllowed(_))
        ));
        // 대소문자가 달라도 로밍은 로밍이다.
        assert!(managed_install_root(WIN, Path::new(r"C:\Users\kim\appdata\roaming\fc")).is_err());
    }

    #[test]
    fn macos_경로는_로밍_규칙과_무관하다() {
        let base = Path::new("/Users/kim/Library/Application Support/file-converter");

        assert!(managed_install_root(MAC, base).is_ok());
    }
}
