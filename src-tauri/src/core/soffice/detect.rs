//! soffice 실행 파일 탐지 — 후보 경로 생성과 채택.
//!
//! 두 단계로 나눈다. `candidates_*` 는 파일시스템을 보지 않고 "볼 만한 경로"를 우선순위대로
//! 만들고, [`detect`] 가 존재 확인과 `--version` 실행으로 실제 설치본을 고른다.
//! 덕분에 macOS 개발 머신에서도 Windows·Linux 후보 로직을 그대로 테스트할 수 있다.
//!
//! 라이선스 경계: LibreOffice 는 외부 프로세스로만 다룬다 — 여기서는 경로만 조립한다.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::probe::{Hive, RegView, SofficeProbe};
use super::profile::ProfileUrl;
use super::runner::{ProcessRequest, ProcessRunner, Termination};
use super::version::{parse_version, LoVersion};

/// 후보 경로가 어디서 나왔는지. 진단 메시지와 우선순위 검증에 쓴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    UserOverride,
    Managed,
    Registry,
    StandardPath,
    Homebrew,
    PathLookup,
}

/// 아직 존재 여부를 확인하지 않은 "볼 만한 경로".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub exe: PathBuf,
    pub origin: Origin,
}

/// `--version` 까지 확인해 실제로 쓸 수 있다고 판정된 설치본.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SofficeInfo {
    pub exe: PathBuf,
    pub origin: Origin,
    pub version: LoVersion,
}

/// 유닉스 계열 실행 파일 이름.
const UNIX_NAME: &str = "soffice";

/// Windows 실행 파일 이름 (우선순위 순).
///
/// `soffice.exe` 는 `DETACHED_PROCESS` 로 자식을 띄워 stdout/stderr 를 전혀 물려주지 않는다.
/// 콘솔 런처인 `soffice.com` 만 출력을 캡처할 수 있으므로 항상 먼저 본다.
const WINDOWS_NAMES: &[&str] = &["soffice.com", "soffice.exe"];

/// 호스트 플랫폼에서 찾을 실행 파일 이름 (우선순위 순).
#[cfg(windows)]
pub const SOFFICE_NAMES: &[&str] = WINDOWS_NAMES;
#[cfg(not(windows))]
pub const SOFFICE_NAMES: &[&str] = &[UNIX_NAME];

/// macOS 앱 번들 안의 실제 실행 파일 위치. 번들에는 `Contents/program/` 이 없다.
const MACOS_BUNDLE_EXE: &str = "LibreOffice.app/Contents/MacOS/soffice";

/// Homebrew 가 깔아주는 `soffice` — 심볼릭 링크가 아니라 bash 래퍼이므로 그대로 실행한다.
const MACOS_HOMEBREW_EXES: &[&str] = &["/opt/homebrew/bin/soffice", "/usr/local/bin/soffice"];

/// `ShellExecuteEx` 전용 키지만 설치 경로를 읽는 용도로는 쓸 만하다.
pub const APP_PATHS_SUBKEY: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\soffice.exe";

/// 32비트 LibreOffice 가 여전히 배포되므로 `(x86)` 까지 모두 본다.
const WINDOWS_PROGRAM_FILES_VARS: &[&str] = &["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"];

const WINDOWS_INSTALL_SUBDIR: &str = r"LibreOffice\program";

/// per-user MSI 설치는 HKCU 에 쓰이고, 32비트 설치는 WOW6432 뷰에만 보인다.
const REGISTRY_HIVES: &[Hive] = &[Hive::LocalMachine, Hive::CurrentUser];
const REGISTRY_VIEWS: &[RegView] = &[RegView::Bits64, RegView::Bits32];

const LINUX_STANDARD_DIRS: &[&str] = &[
    "/usr/bin",
    "/usr/lib/libreoffice/program",
    "/usr/lib64/libreoffice/program",
    "/opt/libreoffice/program",
];

/// `--version` 은 즉시 끝나야 한다. 넘기면 설치본이 망가진 것으로 본다.
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(20);

/// 우선순위를 지키면서 같은 경로를 두 번 담지 않는 수집기.
#[derive(Debug, Default)]
struct CandidateList {
    items: Vec<Candidate>,
    seen: BTreeSet<PathBuf>,
}

impl CandidateList {
    fn push(&mut self, exe: impl Into<PathBuf>, origin: Origin) {
        let exe = exe.into();
        if self.seen.insert(exe.clone()) {
            self.items.push(Candidate { exe, origin });
        }
    }

    /// Windows 폴더 하나에서 `.com` → `.exe` 순으로 후보를 만든다.
    fn push_windows_dir(&mut self, dir: &str, origin: Origin) {
        for name in WINDOWS_NAMES {
            self.push(windows_join(dir, name), origin);
        }
    }

    fn into_vec(self) -> Vec<Candidate> {
        self.items
    }
}

fn push_user_override<P: SofficeProbe + ?Sized>(list: &mut CandidateList, probe: &P) {
    if let Some(path) = probe.user_override() {
        list.push(path, Origin::UserOverride);
    }
}

/// Windows 경로 문자열의 마지막 구분자 앞부분. 유닉스 호스트에서도 동작해야 하므로
/// `Path::parent` 대신 문자열로 자른다 (유닉스에서는 `\` 가 구분자가 아니다).
fn windows_parent(path: &str) -> Option<&str> {
    let index = path.rfind(['\\', '/'])?;
    Some(&path[..index])
}

fn windows_join(dir: &str, child: &str) -> String {
    format!("{}\\{child}", dir.trim_end_matches(['\\', '/']))
}

pub fn candidates_macos<P: SofficeProbe + ?Sized>(probe: &P) -> Vec<Candidate> {
    let mut list = CandidateList::default();
    push_user_override(&mut list, probe);

    if let Some(root) = probe.managed_root() {
        list.push(root.join(MACOS_BUNDLE_EXE), Origin::Managed);
    }

    list.push(
        Path::new("/Applications").join(MACOS_BUNDLE_EXE),
        Origin::StandardPath,
    );
    if let Some(home) = probe.home_dir() {
        list.push(
            home.join("Applications").join(MACOS_BUNDLE_EXE),
            Origin::StandardPath,
        );
    }

    for exe in MACOS_HOMEBREW_EXES {
        list.push(*exe, Origin::Homebrew);
    }

    // GUI 앱(launchd)의 PATH 는 `/usr/bin:/bin:/usr/sbin:/sbin` 뿐이라 반드시 마지막이다.
    if let Some(path) = probe.find_in_path(UNIX_NAME) {
        list.push(path, Origin::PathLookup);
    }

    list.into_vec()
}

pub fn candidates_windows<P: SofficeProbe + ?Sized>(probe: &P) -> Vec<Candidate> {
    let mut list = CandidateList::default();
    push_user_override(&mut list, probe);

    if let Some(root) = probe.managed_root() {
        let program_dir = windows_join(&root.to_string_lossy(), "program");
        list.push_windows_dir(&program_dir, Origin::Managed);
    }

    push_registry_candidates(&mut list, probe);

    for var in WINDOWS_PROGRAM_FILES_VARS {
        let Some(base) = probe.env_var(var) else {
            continue;
        };
        list.push_windows_dir(
            &windows_join(&base, WINDOWS_INSTALL_SUBDIR),
            Origin::StandardPath,
        );
    }

    for name in WINDOWS_NAMES {
        if let Some(path) = probe.find_in_path(name) {
            list.push(path, Origin::PathLookup);
        }
    }

    list.into_vec()
}

/// App Paths 의 (Default) 값은 `soffice.exe` 전체 경로다 — 폴더만 떼어 `.com` 을 먼저 노린다.
fn push_registry_candidates<P: SofficeProbe + ?Sized>(list: &mut CandidateList, probe: &P) {
    for hive in REGISTRY_HIVES {
        for view in REGISTRY_VIEWS {
            let Some(value) = probe.registry_string(*hive, APP_PATHS_SUBKEY, "", *view) else {
                continue;
            };
            let Some(dir) = windows_parent(value.trim().trim_matches('"')) else {
                continue;
            };
            list.push_windows_dir(dir, Origin::Registry);
        }
    }
}

pub fn candidates_linux<P: SofficeProbe + ?Sized>(probe: &P) -> Vec<Candidate> {
    let mut list = CandidateList::default();
    push_user_override(&mut list, probe);

    if let Some(root) = probe.managed_root() {
        list.push(root.join("program").join(UNIX_NAME), Origin::Managed);
    }

    for dir in LINUX_STANDARD_DIRS {
        list.push(Path::new(dir).join(UNIX_NAME), Origin::StandardPath);
    }

    if let Some(path) = probe.find_in_path(UNIX_NAME) {
        list.push(path, Origin::PathLookup);
    }

    list.into_vec()
}

#[cfg(target_os = "macos")]
pub fn candidates<P: SofficeProbe + ?Sized>(probe: &P) -> Vec<Candidate> {
    candidates_macos(probe)
}

#[cfg(windows)]
pub fn candidates<P: SofficeProbe + ?Sized>(probe: &P) -> Vec<Candidate> {
    candidates_windows(probe)
}

#[cfg(all(not(target_os = "macos"), not(windows)))]
pub fn candidates<P: SofficeProbe + ?Sized>(probe: &P) -> Vec<Candidate> {
    candidates_linux(probe)
}

/// `--version` 호출 argv. 탐지 단계에서도 프로필을 격리해야 사용자 프로필이 오염되지 않는다.
pub fn version_args(profile: &ProfileUrl) -> Vec<OsString> {
    vec![
        profile.as_arg(),
        OsString::from("--headless"),
        OsString::from("--version"),
    ]
}

/// `unopkg` 는 별도로 탐지하지 않고 soffice 와 같은 디렉토리에서 조립한다.
pub fn unopkg_next_to(soffice: &Path) -> PathBuf {
    let raw = soffice.to_string_lossy();
    let name = if is_windows_exe_name(&raw) {
        // soffice 와 같은 이유로 콘솔 런처를 쓴다.
        "unopkg.com"
    } else {
        "unopkg"
    };

    match raw.rfind(['\\', '/']) {
        Some(index) => PathBuf::from(format!("{}{name}", &raw[..=index])),
        None => PathBuf::from(name),
    }
}

fn is_windows_exe_name(path: &str) -> bool {
    let lowered = path.to_ascii_lowercase();
    lowered.ends_with(".com") || lowered.ends_with(".exe")
}

/// 존재하는 후보를 순서대로 `--version` 실행해 첫 성공을 채택한다.
pub fn detect<P, R>(probe: &P, runner: &R, profile: &ProfileUrl) -> Option<SofficeInfo>
where
    P: SofficeProbe + ?Sized,
    R: ProcessRunner + ?Sized,
{
    detect_among(candidates(probe), probe, runner, profile)
}

/// 후보 목록을 직접 받는 내부 구현 — 어느 호스트에서도 플랫폼별 목록을 검증할 수 있다.
fn detect_among<P, R>(
    candidates: Vec<Candidate>,
    probe: &P,
    runner: &R,
    profile: &ProfileUrl,
) -> Option<SofficeInfo>
where
    P: SofficeProbe + ?Sized,
    R: ProcessRunner + ?Sized,
{
    let args = version_args(profile);

    for candidate in candidates {
        if !probe.is_executable_file(&candidate.exe) {
            continue;
        }

        let request = ProcessRequest {
            program: candidate.exe.clone(),
            args: args.clone(),
            env: Vec::new(),
            timeout: VERSION_TIMEOUT,
        };
        let Ok(output) = runner.run(&request) else {
            continue;
        };
        if output.termination == Termination::TimedOut {
            continue;
        }

        // 26.2 미만은 종료 코드를 못 믿으므로 버전 문자열이 나왔는지만 본다.
        let Some(version) = parse_version(&output.stdout).or_else(|| parse_version(&output.stderr))
        else {
            continue;
        };

        return Some(SofficeInfo {
            exe: candidate.exe,
            origin: candidate.origin,
            version,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::soffice::probe::fake::FakeProbe;
    use crate::core::soffice::probe::{Hive, RegView};
    use crate::core::soffice::runner::fake::{ok_output, FakeRunner};
    use crate::core::soffice::runner::{ProcessOutput, Termination};

    const APP_PATHS: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\soffice.exe";
    const MAC_SYSTEM: &str = "/Applications/LibreOffice.app/Contents/MacOS/soffice";
    const WIN_PROGRAM_DIR: &str = r"C:\Program Files\LibreOffice\program";
    const VERSION_STDOUT: &str = "LibreOffice 26.2.5.2 f1a2b3c\n";

    fn profile() -> ProfileUrl {
        ProfileUrl::from_path_str("/tmp/fc-profile", false).expect("절대 경로")
    }

    /// 경로 구분자는 호스트마다 다르다(Windows 에서 `join` 은 역슬래시를 넣는다).
    /// 문자열로 비교하기 전에 슬래시로 통일해 어느 러너에서도 같은 결과가 나오게 한다.
    fn normalize(path: &str) -> String {
        path.replace('\\', "/")
    }

    fn paths(candidates: &[Candidate]) -> Vec<String> {
        candidates
            .iter()
            .map(|candidate| normalize(&candidate.exe.to_string_lossy()))
            .collect()
    }

    fn position_of(candidates: &[Candidate], needle: &str) -> usize {
        let needle = normalize(needle);
        paths(candidates)
            .iter()
            .position(|path| *path == needle)
            .unwrap_or_else(|| panic!("후보에 없음: {needle}"))
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 앱_관리_설치가_시스템_설치보다_먼저_온다() {
        // Arrange
        let probe = FakeProbe::new().managed_root("/Users/kim/Library/fc/lo");

        // Act
        let found = candidates_macos(&probe);

        // Assert
        let managed = "/Users/kim/Library/fc/lo/LibreOffice.app/Contents/MacOS/soffice";
        assert!(position_of(&found, managed) < position_of(&found, MAC_SYSTEM));
        assert_eq!(found[0].origin, Origin::Managed);
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 사용자_지정_경로가_모든_후보보다_우선한다() {
        // Arrange
        let probe = FakeProbe::new()
            .managed_root("/Users/kim/Library/fc/lo")
            .user_override("/Volumes/tools/soffice");

        // Act
        let found = candidates_macos(&probe);

        // Assert
        assert_eq!(found[0].exe, PathBuf::from("/Volumes/tools/soffice"));
        assert_eq!(found[0].origin, Origin::UserOverride);
    }

    #[test]
    fn 윈도_레지스트리_경로는_com_으로_치환해_채택된다() {
        // Arrange
        let com = format!(r"{WIN_PROGRAM_DIR}\soffice.com");
        let probe = FakeProbe::new()
            .registry(
                Hive::LocalMachine,
                APP_PATHS,
                "",
                RegView::Bits64,
                &format!(r"{WIN_PROGRAM_DIR}\soffice.exe"),
            )
            .executable(&com);
        let runner = FakeRunner::new().responding(&com, ok_output(VERSION_STDOUT));

        // Act
        let info = detect_among(candidates_windows(&probe), &probe, &runner, &profile())
            .expect("탐지 성공");

        // Assert
        assert_eq!(info.exe, PathBuf::from(&com));
        assert_eq!(info.origin, Origin::Registry);
        assert_eq!(info.version, LoVersion::new(26, 2, 5, 2));
    }

    #[test]
    fn 레지스트리는_하이브와_뷰_네_조합을_모두_조회한다() {
        // Arrange — 조합마다 서로 다른 폴더를 심어 네 번 다 읽었는지 본다.
        let combos = [
            (Hive::LocalMachine, RegView::Bits64, r"C:\lo-hklm64"),
            (Hive::LocalMachine, RegView::Bits32, r"C:\lo-hklm32"),
            (Hive::CurrentUser, RegView::Bits64, r"C:\lo-hkcu64"),
            (Hive::CurrentUser, RegView::Bits32, r"C:\lo-hkcu32"),
        ];
        let probe = combos
            .iter()
            .fold(FakeProbe::new(), |probe, (hive, view, dir)| {
                probe.registry(*hive, APP_PATHS, "", *view, &format!(r"{dir}\soffice.exe"))
            });

        // Act
        let found = paths(&candidates_windows(&probe));

        // Assert
        for (_, _, dir) in combos {
            assert!(
                found.contains(&normalize(&format!(r"{dir}\soffice.com"))),
                "누락된 조합: {dir}"
            );
        }
    }

    #[test]
    fn 맥에서_홈브루_경로가_path_탐색보다_앞선다() {
        // Arrange — GUI 앱의 PATH 에는 /opt/homebrew/bin 이 없다.
        let probe = FakeProbe::new().on_path("soffice", "/usr/bin/soffice");

        // Act
        let found = candidates_macos(&probe);

        // Assert
        assert!(
            position_of(&found, "/opt/homebrew/bin/soffice")
                < position_of(&found, "/usr/bin/soffice")
        );
        assert_eq!(found.last().expect("후보 있음").origin, Origin::PathLookup);
    }

    #[test]
    fn 맥_후보는_contents_macos_를_쓰고_contents_program_은_없다() {
        // Arrange
        let probe = FakeProbe::new().managed_root("/Users/kim/Library/fc/lo");

        // Act
        let found = paths(&candidates_macos(&probe));

        // Assert
        assert!(found.iter().any(|p| p.ends_with("Contents/MacOS/soffice")));
        assert!(!found.iter().any(|p| p.contains("Contents/program")));
    }

    #[test]
    fn 버전_출력이_깨진_후보는_건너뛰고_다음으로_넘어간다() {
        // Arrange
        let probe = FakeProbe::new()
            .managed_root("/managed")
            .executable("/managed/LibreOffice.app/Contents/MacOS/soffice")
            .executable(MAC_SYSTEM);
        let runner = FakeRunner::new()
            .responding(
                "/managed/LibreOffice.app/Contents/MacOS/soffice",
                ok_output("쓰레기 출력\n"),
            )
            .responding(MAC_SYSTEM, ok_output(VERSION_STDOUT));

        // Act
        let info =
            detect_among(candidates_macos(&probe), &probe, &runner, &profile()).expect("탐지 성공");

        // Assert
        assert_eq!(info.exe, PathBuf::from(MAC_SYSTEM));
        assert_eq!(info.origin, Origin::StandardPath);
    }

    #[test]
    fn 타임아웃된_후보도_건너뛴다() {
        // Arrange
        let probe = FakeProbe::new().executable(MAC_SYSTEM);
        let runner = FakeRunner::new().responding(
            MAC_SYSTEM,
            ProcessOutput {
                termination: Termination::TimedOut,
                stdout: String::new(),
                stderr: String::new(),
            },
        );

        // Act
        let info = detect_among(candidates_macos(&probe), &probe, &runner, &profile());

        // Assert
        assert!(info.is_none());
    }

    #[test]
    fn 아무_설치도_없으면_none() {
        // Arrange
        let probe = FakeProbe::new();
        let runner = FakeRunner::new().default_response(ok_output(VERSION_STDOUT));

        // Act
        let info = detect_among(candidates_macos(&probe), &probe, &runner, &profile());

        // Assert
        assert!(info.is_none());
    }

    #[test]
    fn 윈도에서_com_이_없으면_exe_로_폴백한다() {
        // Arrange
        let exe = format!(r"{WIN_PROGRAM_DIR}\soffice.exe");
        let probe = FakeProbe::new()
            .env("ProgramFiles", r"C:\Program Files")
            .executable(&exe);
        let runner = FakeRunner::new().responding(&exe, ok_output(VERSION_STDOUT));

        // Act
        let info = detect_among(candidates_windows(&probe), &probe, &runner, &profile())
            .expect("탐지 성공");

        // Assert
        assert_eq!(info.exe, PathBuf::from(&exe));
        assert_eq!(info.origin, Origin::StandardPath);
    }

    #[test]
    fn 프로그램파일즈_32비트_후보가_빠지지_않는다() {
        // Arrange — 32비트 LibreOffice 는 아직 배포된다.
        let probe = FakeProbe::new()
            .env("ProgramFiles", r"C:\Program Files")
            .env("ProgramW6432", r"C:\Program Files")
            .env("ProgramFiles(x86)", r"C:\Program Files (x86)");

        // Act
        let found = paths(&candidates_windows(&probe));

        // Assert
        assert!(found.contains(&normalize(
            r"C:\Program Files (x86)\LibreOffice\program\soffice.com"
        )));
        assert!(found.contains(&normalize(
            r"C:\Program Files (x86)\LibreOffice\program\soffice.exe"
        )));
    }

    #[test]
    fn unopkg_는_soffice_형제로_조립된다() {
        // Arrange & Act & Assert
        assert_eq!(
            unopkg_next_to(Path::new(MAC_SYSTEM)),
            PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/unopkg")
        );
        assert_eq!(
            unopkg_next_to(Path::new(&format!(r"{WIN_PROGRAM_DIR}\soffice.com"))),
            PathBuf::from(format!(r"{WIN_PROGRAM_DIR}\unopkg.com"))
        );
        assert_eq!(
            unopkg_next_to(Path::new(&format!(r"{WIN_PROGRAM_DIR}\soffice.exe"))),
            PathBuf::from(format!(r"{WIN_PROGRAM_DIR}\unopkg.com"))
        );
    }

    #[test]
    fn 버전_확인_호출에도_프로필_격리_인자가_붙는다() {
        // Arrange
        let probe = FakeProbe::new().executable(MAC_SYSTEM);
        let runner = FakeRunner::new().responding(MAC_SYSTEM, ok_output(VERSION_STDOUT));

        // Act
        detect_among(candidates_macos(&probe), &probe, &runner, &profile()).expect("탐지 성공");

        // Assert
        let call = runner.calls().into_iter().next().expect("호출 기록");
        assert!(call
            .args
            .iter()
            .any(|arg| arg.to_string_lossy().starts_with("-env:UserInstallation=")));
        assert!(call.args.contains(&OsString::from("--version")));
    }

    #[test]
    fn 버전_인자는_프로필_인자와_version_을_함께_넘긴다() {
        // Arrange & Act
        let args = version_args(&profile());

        // Assert
        assert_eq!(args[0], profile().as_arg());
        assert!(args.contains(&OsString::from("--version")));
        assert!(args.contains(&OsString::from("--headless")));
    }

    #[test]
    fn 실행_파일이_아닌_후보는_러너를_호출조차_하지_않는다() {
        // Arrange — 후보는 많지만 실제 파일은 하나도 없다.
        let probe = FakeProbe::new().managed_root("/managed");
        let runner = FakeRunner::new().default_response(ok_output(VERSION_STDOUT));

        // Act
        let info = detect_among(candidates_macos(&probe), &probe, &runner, &profile());

        // Assert
        assert!(info.is_none());
        assert_eq!(runner.call_count(), 0);
    }

    #[test]
    fn 같은_경로가_두_경로원에서_나와도_중복_후보를_만들지_않는다() {
        // Arrange — 사용자 지정 경로가 표준 설치 경로와 같다.
        let probe = FakeProbe::new().user_override(MAC_SYSTEM);

        // Act
        let found = candidates_macos(&probe);

        // Assert
        let repeats = paths(&found)
            .iter()
            .filter(|p| **p == normalize(MAC_SYSTEM))
            .count();
        assert_eq!(repeats, 1);
        assert_eq!(found[0].origin, Origin::UserOverride);
    }

    #[test]
    fn 채택한_결과의_origin_이_실제_경로원과_일치한다() {
        // Arrange — 홈브루 래퍼만 존재한다.
        let probe = FakeProbe::new().executable("/opt/homebrew/bin/soffice");
        let runner =
            FakeRunner::new().responding("/opt/homebrew/bin/soffice", ok_output(VERSION_STDOUT));

        // Act
        let info =
            detect_among(candidates_macos(&probe), &probe, &runner, &profile()).expect("탐지 성공");

        // Assert
        assert_eq!(info.origin, Origin::Homebrew);
        assert_eq!(info.exe, PathBuf::from("/opt/homebrew/bin/soffice"));
    }

    #[test]
    fn 리눅스_후보는_표준_경로와_path_탐색을_포함한다() {
        // Arrange
        let probe = FakeProbe::new()
            .managed_root("/home/kim/.local/share/fc/lo")
            .on_path("soffice", "/usr/local/bin/soffice");

        // Act
        let found = candidates_linux(&probe);
        let listed = paths(&found);

        // Assert
        assert_eq!(
            found[0].exe,
            PathBuf::from("/home/kim/.local/share/fc/lo/program/soffice")
        );
        assert!(listed.contains(&"/usr/bin/soffice".to_string()));
        assert!(listed.contains(&"/usr/lib/libreoffice/program/soffice".to_string()));
        assert_eq!(found.last().expect("후보 있음").origin, Origin::PathLookup);
    }

    #[test]
    fn 실행_이름은_윈도에서_com_을_먼저_본다() {
        // Arrange — soffice.exe 는 stdout 을 물려주지 않는다.
        if cfg!(windows) {
            assert_eq!(SOFFICE_NAMES, &["soffice.com", "soffice.exe"]);
        } else {
            assert_eq!(SOFFICE_NAMES, &["soffice"]);
        }
    }
}
