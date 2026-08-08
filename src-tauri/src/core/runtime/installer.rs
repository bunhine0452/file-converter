//! 내려받은 자산을 앱 데이터 디렉토리에 푸는 설치기.
//!
//! 실제 설치는 OS 도구(hdiutil/ditto/msiexec/tar)를 **외부 프로세스로** 부른다.
//! 여기서는 경로 규칙과 argv 조립만 순수 함수로 두어, 도구 없이도 검증 가능하게 한다.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::core::fs_port::FileSystem;
use crate::core::runtime::assets::{Os, JRE_VERSION};

/// 앱이 확장을 풀어 넣는 디렉토리 이름.
pub const EXTENSION_DIR_NAME: &str = "H2Orestart";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InstallError {
    #[error("압축을 푸는 데 실패했습니다: {0}")]
    Extract(String),
    #[error("설치 도구가 실패했습니다: {0}")]
    Tool(String),
    #[error("설치 결과에서 {0} 를 찾지 못했습니다")]
    NotFoundAfterInstall(String),
}

pub trait ToolInstaller: Send + Sync {
    /// dmg/msi 를 풀어 LibreOffice 설치 루트를 돌려준다.
    fn install_libreoffice(&self, archive: &Path, dest: &Path) -> Result<PathBuf, InstallError>;
    /// tar.gz/zip 을 풀어 `JAVA_HOME` 을 돌려준다.
    fn install_jre(&self, archive: &Path, dest: &Path) -> Result<PathBuf, InstallError>;
    /// `.oxt`(zip) 를 지정 디렉토리로 푼다.
    fn unpack_oxt(&self, oxt: &Path, dest: &Path) -> Result<(), InstallError>;
}

// ── macOS ────────────────────────────────────────────────────────

/// dmg 를 마운트한다. `-plist` 로 마운트 지점을 기계적으로 읽고,
/// `-nobrowse` 로 Finder 에 튀어나오지 않게 한다.
pub fn hdiutil_attach_args(dmg: &Path) -> Vec<OsString> {
    vec![
        OsString::from("attach"),
        OsString::from("-plist"),
        OsString::from("-nobrowse"),
        OsString::from("-readonly"),
        dmg.as_os_str().to_os_string(),
    ]
}

pub fn hdiutil_detach_args(mount_point: &Path) -> Vec<OsString> {
    vec![
        OsString::from("detach"),
        // 복사 직후에도 핸들이 남아 있을 수 있어 강제로 뗀다.
        OsString::from("-force"),
        mount_point.as_os_str().to_os_string(),
    ]
}

/// `hdiutil attach -plist` 출력에서 마운트 지점을 뽑는다.
///
/// plist 파서를 들이지 않고 `<key>mount-point</key>` 바로 뒤의 `<string>` 만 읽는다 —
/// 우리가 필요한 건 이 값 하나뿐이고, 형식이 안정적이다.
pub fn parse_hdiutil_mount_point(plist: &str) -> Option<PathBuf> {
    let after_key = plist.split("<key>mount-point</key>").nth(1)?;
    let open = after_key.find("<string>")? + "<string>".len();
    let rest = &after_key[open..];
    let close = rest.find("</string>")?;
    let value = rest[..close].trim();

    if value.is_empty() {
        return None;
    }

    Some(PathBuf::from(value))
}

/// `.app` 번들 복사는 `cp` 가 아니라 `ditto` 로 한다 — 리소스 포크와 권한을 보존한다.
pub fn ditto_args(source: &Path, dest: &Path) -> Vec<OsString> {
    vec![
        source.as_os_str().to_os_string(),
        dest.as_os_str().to_os_string(),
    ]
}

/// 다운로드한 번들에 붙은 격리 속성을 떼지 않으면 Gatekeeper 가 실행을 막는다.
pub fn xattr_clear_quarantine_args(target: &Path) -> Vec<OsString> {
    vec![
        OsString::from("-dr"),
        OsString::from("com.apple.quarantine"),
        target.as_os_str().to_os_string(),
    ]
}

// ── Windows ──────────────────────────────────────────────────────

/// 관리자 권한 없이 사용자 디렉토리에 푸는 administrative install.
pub fn msiexec_admin_args(msi: &Path, target_dir: &Path) -> Vec<OsString> {
    // TARGETDIR 은 값과 붙인 인자 하나여야 한다 — 쪼개면 공백 있는 경로가 깨진다.
    let mut target = OsString::from("TARGETDIR=");
    target.push(target_dir.as_os_str());

    vec![
        OsString::from("/a"),
        msi.as_os_str().to_os_string(),
        target,
        OsString::from("/qn"),
    ]
}

// ── 설치 트리 안에서 찾기 ─────────────────────────────────────────

/// 설치 루트에서 soffice 가 있을 만한 곳을 우선순위대로.
/// 여러 개인 이유는 배포 형태(dmg 번들 / msi 관리 설치)마다 트리가 다르기 때문이다.
pub fn soffice_candidates(install_root: &Path, os: Os) -> Vec<PathBuf> {
    match os {
        Os::MacOs => vec![
            install_root
                .join("LibreOffice.app")
                .join("Contents")
                .join("MacOS")
                .join("soffice"),
            install_root
                .join("Applications")
                .join("LibreOffice.app")
                .join("Contents")
                .join("MacOS")
                .join("soffice"),
        ],
        // msiexec /a 는 배포본에 따라 한 겹 더 들어간 트리를 만든다.
        Os::Windows => ["", "LibreOffice"]
            .into_iter()
            .flat_map(|nested| {
                let program = if nested.is_empty() {
                    install_root.join("program")
                } else {
                    install_root.join(nested).join("program")
                };
                ["soffice.com", "soffice.exe"]
                    .into_iter()
                    .map(move |name| program.join(name))
            })
            .collect(),
    }
}

/// unopkg 는 soffice 와 같은 디렉토리에 있다 (확장자도 따라간다).
pub fn unopkg_for(soffice: &Path) -> PathBuf {
    let directory = soffice.parent().unwrap_or(Path::new(""));
    let extension = soffice.extension().and_then(|e| e.to_str());

    match extension {
        Some(extension) => directory.join(format!("unopkg.{extension}")),
        None => directory.join("unopkg"),
    }
}

/// 앱이 설치한 LibreOffice 에 확장을 직접 넣을 디렉토리.
///
/// 실행 파일 위치가 예상 트리와 다르면 추정하지 않고 `None` 을 돌려준다 —
/// 엉뚱한 곳에 풀면 확장이 등록되지 않은 채 조용히 실패한다.
pub fn bundled_extension_dir(soffice: &Path, os: Os) -> Option<PathBuf> {
    let install_root = match os {
        // <root>/LibreOffice.app/Contents/MacOS/soffice → <root>/LibreOffice.app/Contents
        Os::MacOs => soffice.parent()?.parent()?,
        // <root>/program/soffice.com → <root>
        Os::Windows => soffice.parent()?.parent()?,
    };

    if install_root.as_os_str().is_empty() || install_root == Path::new("/") {
        return None;
    }

    let extensions = match os {
        Os::MacOs => install_root.join("Resources").join("extensions"),
        Os::Windows => install_root.join("share").join("extensions"),
    };

    Some(extensions.join(EXTENSION_DIR_NAME))
}

/// 앱이 설치한 LibreOffice 가 기동할 때 읽는 글꼴 디렉토리.
///
/// 사용자 시스템 글꼴 폴더는 절대 건드리지 않는다 — 우리가 설치한 LibreOffice 안에만 둔다.
pub fn bundled_font_dir(soffice: &Path, os: Os) -> Option<PathBuf> {
    let install_root = soffice.parent()?.parent()?;
    if install_root.as_os_str().is_empty() || install_root == Path::new("/") {
        return None;
    }

    let fonts = match os {
        Os::MacOs => install_root.join("Resources").join("fonts"),
        Os::Windows => install_root.join("share").join("fonts"),
    };

    Some(fonts.join("truetype"))
}

/// 압축을 푼 JRE 트리에서 `JAVA_HOME` 후보를 우선순위대로.
/// 아카이브가 최상위 디렉토리 없이 풀리는 배포본에 대비해 루트 자체도 포함한다.
pub fn java_home_candidates(extracted_root: &Path, os: Os) -> Vec<PathBuf> {
    let versioned = extracted_root.join(jre_dir_name());

    match os {
        Os::MacOs => vec![
            versioned.join("Contents").join("Home"),
            versioned,
            extracted_root.join("Contents").join("Home"),
            extracted_root.to_path_buf(),
        ],
        Os::Windows => vec![versioned, extracted_root.to_path_buf()],
    }
}

/// Temurin 아카이브가 만드는 최상위 디렉토리 이름 (`21.0.12+8` → `jdk-21.0.12+8-jre`).
pub fn jre_dir_name() -> String {
    format!("jdk-{JRE_VERSION}-jre")
}

/// `JAVA_HOME` 이 진짜인지 확인할 때 보는 실행 파일.
pub fn java_executable(java_home: &Path, os: Os) -> PathBuf {
    let name = match os {
        Os::MacOs => "java",
        Os::Windows => "java.exe",
    };

    java_home.join("bin").join(name)
}

/// 압축을 푼 트리에서 실제로 쓸 수 있는 `JAVA_HOME` 을 고른다.
///
/// **디렉토리가 있다는 것만으로 인정하면 안 된다.** 다운로드가 중간에 끊겨 빈 폴더만
/// 남은 경우 이를 준비됨으로 오해하고 JRE 설치를 영영 건너뛰게 되며, 그 상태로 초기화된
/// soffice 프로필은 이후 계속 `source file could not be loaded` 로 실패한다.
pub fn resolve_java_home(root: &Path, os: Os, fs: &dyn FileSystem) -> Option<PathBuf> {
    java_home_candidates(root, os)
        .into_iter()
        .find(|candidate| fs.is_file(&java_executable(candidate, os)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    /// 경로 구분자는 호스트마다 다르다 — 문자열 비교 전에 슬래시로 통일한다.
    fn normalize(path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }

    const ATTACH_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>system-entities</key>
  <array>
    <dict>
      <key>content-hint</key><string>GUID_partition_scheme</string>
      <key>dev-entry</key><string>/dev/disk4</string>
    </dict>
    <dict>
      <key>dev-entry</key><string>/dev/disk4s1</string>
      <key>mount-point</key><string>/Volumes/LibreOffice</string>
      <key>volume-kind</key><string>hfs</string>
    </dict>
  </array>
</dict>
</plist>"#;

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn dmg_마운트는_plist_와_nobrowse_로_한다() {
        let args = rendered(&hdiutil_attach_args(Path::new("/tmp/lo.dmg")));

        assert_eq!(args[0], "attach");
        assert!(args.iter().any(|a| a == "-plist"));
        assert!(args.iter().any(|a| a == "-nobrowse"));
        assert!(args.iter().any(|a| a == "/tmp/lo.dmg"));
    }

    #[test]
    fn plist_에서_마운트_지점을_읽는다() {
        assert_eq!(
            parse_hdiutil_mount_point(ATTACH_PLIST),
            Some(PathBuf::from("/Volumes/LibreOffice"))
        );
    }

    #[test]
    fn 맥_설치_루트에서_앱_번들_실행파일을_찾는다() {
        let candidates = soffice_candidates(Path::new("/data/runtime/lo"), Os::MacOs);

        assert_eq!(
            normalize(&candidates[0]),
            "/data/runtime/lo/LibreOffice.app/Contents/MacOS/soffice"
        );
    }

    #[test]
    fn 윈도_설치_루트에서는_com_을_먼저_본다() {
        let candidates = soffice_candidates(Path::new("/data/runtime/lo"), Os::Windows);
        let listed: Vec<String> = candidates.iter().map(|path| normalize(path)).collect();

        let com = listed
            .iter()
            .position(|p| p.ends_with("/program/soffice.com"))
            .expect("com 후보");
        let exe = listed
            .iter()
            .position(|p| p.ends_with("/program/soffice.exe"))
            .expect("exe 후보");
        assert!(com < exe, "stdout 을 캡처하려면 .com 이 먼저여야 한다");
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 마운트_지점이_없는_plist_는_none() {
        let plist = r#"<plist><dict><key>system-entities</key><array>
            <dict><key>dev-entry</key><string>/dev/disk9</string></dict>
        </array></dict></plist>"#;

        assert_eq!(parse_hdiutil_mount_point(plist), None);
    }

    #[test]
    fn 빈_출력이나_깨진_plist_는_none() {
        assert_eq!(parse_hdiutil_mount_point(""), None);
        assert_eq!(
            parse_hdiutil_mount_point("<plist><key>mount-point</key>"),
            None
        );
    }

    #[test]
    fn 마운트_해제는_강제_옵션을_붙인다() {
        // 변환 직후 파일 핸들이 남아 있으면 일반 detach 는 실패한다.
        let args = rendered(&hdiutil_detach_args(Path::new("/Volumes/LibreOffice")));

        assert_eq!(args[0], "detach");
        assert!(args.iter().any(|a| a == "-force"));
        assert!(args.iter().any(|a| a == "/Volumes/LibreOffice"));
    }

    #[test]
    fn ditto_는_원본과_대상을_그대로_넘긴다() {
        let args = rendered(&ditto_args(
            Path::new("/Volumes/LibreOffice/LibreOffice.app"),
            Path::new("/data/runtime/lo/LibreOffice.app"),
        ));

        assert_eq!(
            args,
            vec![
                "/Volumes/LibreOffice/LibreOffice.app".to_string(),
                "/data/runtime/lo/LibreOffice.app".to_string()
            ]
        );
    }

    #[test]
    fn 격리_속성은_재귀적으로_제거한다() {
        let args = rendered(&xattr_clear_quarantine_args(Path::new("/data/lo.app")));

        assert!(args.iter().any(|a| a == "-dr"));
        assert!(args.iter().any(|a| a == "com.apple.quarantine"));
        assert!(args.iter().any(|a| a == "/data/lo.app"));
    }

    #[test]
    fn msiexec_는_관리_설치와_무인_모드로_연다() {
        let args = rendered(&msiexec_admin_args(
            Path::new(r"C:\tmp\lo.msi"),
            Path::new(r"C:\Users\kim\AppData\Local\fc\runtime\lo"),
        ));

        assert!(args.iter().any(|a| a == "/a"));
        assert!(args.iter().any(|a| a == "/qn"));
        assert!(args.iter().any(|a| a == r"C:\tmp\lo.msi"));
        // TARGETDIR 은 값과 붙은 한 개의 인자여야 한다 (공백이 있어도 쪼개지지 않게).
        assert!(args
            .iter()
            .any(|a| a == r"TARGETDIR=C:\Users\kim\AppData\Local\fc\runtime\lo"));
    }

    #[test]
    fn 공백이_있는_경로도_인자_하나로_유지된다() {
        let args = msiexec_admin_args(
            Path::new(r"C:\tmp\lo.msi"),
            Path::new(r"C:\Program Files\fc"),
        );

        assert!(args
            .iter()
            .any(|a| a.to_string_lossy() == r"TARGETDIR=C:\Program Files\fc"));
        assert_eq!(args.len(), 4, "인자가 공백에서 쪼개지면 안 된다: {args:?}");
    }

    #[test]
    fn unopkg_는_soffice_형제로_조립된다() {
        assert_eq!(
            normalize(&unopkg_for(Path::new(
                "/data/lo/LibreOffice.app/Contents/MacOS/soffice"
            ))),
            "/data/lo/LibreOffice.app/Contents/MacOS/unopkg"
        );
        assert_eq!(
            normalize(&unopkg_for(Path::new("/data/lo/program/soffice.com"))),
            "/data/lo/program/unopkg.com"
        );
        assert_eq!(
            normalize(&unopkg_for(Path::new("/data/lo/program/soffice.exe"))),
            "/data/lo/program/unopkg.exe"
        );
    }

    #[test]
    fn 맥_번들_확장_디렉토리는_contents_resources_아래다() {
        let dir = bundled_extension_dir(
            Path::new("/data/lo/LibreOffice.app/Contents/MacOS/soffice"),
            Os::MacOs,
        )
        .expect("확장 디렉토리");

        assert_eq!(
            normalize(&dir),
            "/data/lo/LibreOffice.app/Contents/Resources/extensions/H2Orestart"
        );
    }

    #[test]
    fn 윈도_번들_확장_디렉토리는_share_아래다() {
        let dir = bundled_extension_dir(Path::new("/data/lo/program/soffice.com"), Os::Windows)
            .expect("확장 디렉토리");

        assert_eq!(normalize(&dir), "/data/lo/share/extensions/H2Orestart");
    }

    #[test]
    fn 예상_밖의_실행파일_위치면_확장_디렉토리를_추정하지_않는다() {
        assert_eq!(
            bundled_extension_dir(Path::new("soffice"), Os::Windows),
            None
        );
        assert_eq!(
            bundled_extension_dir(Path::new("/soffice"), Os::MacOs),
            None
        );
    }

    #[test]
    fn 맥_java_home_은_contents_home_이다() {
        let candidates = java_home_candidates(Path::new("/data/runtime/jre"), Os::MacOs);

        assert_eq!(
            normalize(&candidates[0]),
            format!("/data/runtime/jre/{}/Contents/Home", jre_dir_name())
        );
    }

    #[test]
    fn 윈도_java_home_은_버전_디렉토리_자체다() {
        let candidates = java_home_candidates(Path::new("/data/runtime/jre"), Os::Windows);

        assert_eq!(
            normalize(&candidates[0]),
            format!("/data/runtime/jre/{}", jre_dir_name())
        );
    }

    #[test]
    fn java_home_후보에_압축해제_루트_자체도_포함한다() {
        // 아카이브가 최상위 디렉토리 없이 풀리는 배포본에 대비한다.
        for os in [Os::MacOs, Os::Windows] {
            let candidates = java_home_candidates(Path::new("/data/runtime/jre"), os);
            let listed: Vec<String> = candidates.iter().map(|path| normalize(path)).collect();

            assert!(
                listed
                    .iter()
                    .any(|p| p == "/data/runtime/jre" || p == "/data/runtime/jre/Contents/Home"),
                "루트 폴백이 없다: {listed:?}"
            );
        }
    }

    #[test]
    fn 빈_디렉토리는_java_home_으로_인정하지_않는다() {
        // 다운로드가 끊겨 폴더만 남으면 준비됨으로 오해해 JRE 설치를 영영 건너뛴다.
        use crate::core::fs_port::fake::FakeFs;

        let fs = FakeFs::new().with_dir("/data/jre");

        assert_eq!(
            resolve_java_home(Path::new("/data/jre"), Os::MacOs, &fs),
            None
        );
    }

    #[test]
    fn java_실행파일이_있는_후보만_java_home_이_된다() {
        use crate::core::fs_port::fake::FakeFs;

        let home = format!("/data/jre/{}/Contents/Home", jre_dir_name());
        let fs = FakeFs::new().with_file(format!("{home}/bin/java"), b"bin".to_vec());

        assert_eq!(
            resolve_java_home(Path::new("/data/jre"), Os::MacOs, &fs),
            Some(PathBuf::from(home))
        );
    }

    #[test]
    fn 윈도는_java_exe_를_본다() {
        use crate::core::fs_port::fake::FakeFs;

        let home = format!("/data/jre/{}", jre_dir_name());
        // 확장자 없는 java 만 있으면 윈도에서는 인정하지 않는다.
        let unix_only = FakeFs::new().with_file(format!("{home}/bin/java"), b"bin".to_vec());
        assert_eq!(
            resolve_java_home(Path::new("/data/jre"), Os::Windows, &unix_only),
            None
        );

        let windows = FakeFs::new().with_file(format!("{home}/bin/java.exe"), b"bin".to_vec());
        assert_eq!(
            resolve_java_home(Path::new("/data/jre"), Os::Windows, &windows),
            Some(PathBuf::from(home))
        );
    }

    #[test]
    fn jre_디렉토리_이름은_pin_한_버전을_따른다() {
        assert_eq!(jre_dir_name(), format!("jdk-{JRE_VERSION}-jre"));
        assert!(jre_dir_name().starts_with("jdk-"));
        assert!(jre_dir_name().ends_with("-jre"));
    }

    // ── 글꼴 디렉토리 ─────────────────────────────────────────────

    #[test]
    fn 맥은_앱_번들_안의_글꼴_폴더를_쓴다() {
        let dir = bundled_font_dir(
            Path::new("/data/runtime/libreoffice/LibreOffice.app/Contents/MacOS/soffice"),
            Os::MacOs,
        );

        assert_eq!(
            dir,
            Some(PathBuf::from(
                "/data/runtime/libreoffice/LibreOffice.app/Contents/Resources/fonts/truetype"
            ))
        );
    }

    #[test]
    fn 윈도는_설치_루트의_share_아래를_쓴다() {
        // 호스트 구분자 규칙을 타므로 기존 테스트와 같은 방식으로 검사한다.
        let dir =
            bundled_font_dir(Path::new("/data/lo/program/soffice.com"), Os::Windows).expect("경로");

        assert_eq!(normalize(&dir), "/data/lo/share/fonts/truetype");
    }

    #[test]
    fn 루트에_붙은_경로는_거절한다() {
        // 시스템 글꼴 폴더에 쓰는 사고를 막는다.
        assert_eq!(bundled_font_dir(Path::new("/soffice"), Os::MacOs), None);
    }
}
