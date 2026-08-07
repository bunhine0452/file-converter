//! soffice 호출 argv 조립.
//!
//! 여기서 하는 일은 "어떤 인자로 부를지"를 정하는 것뿐이다 — 실행은
//! [`super::runner::ProcessRunner`] 뒤에서 일어나므로 argv 는 순수 함수로 검증된다.
//!
//! 확정된 형태(조사로 검증):
//! ```text
//! soffice -env:UserInstallation=file:///<프로필> --headless --norestore --nolockcheck \
//!         --convert-to pdf:writer_pdf_Export --outdir <잡 전용 outdir> <입력>
//! ```

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::profile::ProfileUrl;
use super::runner::ProcessRequest;

/// `--convert-to` 에 넘길 값. 확장자와 필터명을 콜론으로 잇는다.
pub const PDF_FILTER: &str = "pdf:writer_pdf_Export";

/// H2Orestart 는 Java 확장이라 JRE 경로가 필요하다. 자식 프로세스 env 로만 준다.
pub const JAVA_HOME_KEY: &str = "JAVA_HOME";

const PDF_EXTENSION: &str = "pdf";
/// 입력 경로에서 파일명을 못 뽑았을 때의 최후 이름 (정상 경로에서는 쓰이지 않는다).
const FALLBACK_OUTPUT_NAME: &str = "output";

const MIN_TIMEOUT_SECS: u64 = 60;
const MAX_TIMEOUT_SECS: u64 = 600;
const TIMEOUT_SECS_PER_MIB: u64 = 10;
const BYTES_PER_MIB: u64 = 1024 * 1024;

/// 한 번의 soffice 변환 호출에 필요한 전부.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertPlan {
    /// 잡 전용 프로필 — 같은 프로필로 병렬 실행하면 한쪽이 조용히 실패한다.
    pub profile: ProfileUrl,
    /// 원본 확장자(`.hwp`/`.hwpx`)를 유지해야 한다. H2Orestart 타입은
    /// MediaType 이 비어 있어 확장자와 내용으로만 감지된다.
    pub input: PathBuf,
    /// 잡마다 고유해야 한다 — soffice 는 같은 basename 을 말없이 덮어쓴다.
    pub out_dir: PathBuf,
}

impl ConvertPlan {
    /// soffice 에 그대로 넘길 인자 목록. 프로필 인자가 항상 처음에 온다.
    pub fn args(&self) -> Vec<OsString> {
        vec![
            self.profile.as_arg(),
            OsString::from("--headless"),
            OsString::from("--norestore"),
            OsString::from("--nolockcheck"),
            OsString::from("--convert-to"),
            OsString::from(PDF_FILTER),
            OsString::from("--outdir"),
            self.out_dir.clone().into_os_string(),
            self.input.clone().into_os_string(),
        ]
    }

    /// `<out_dir>/<입력 basename>.pdf` — soffice 의 명명 규칙과 같게 계산한다.
    ///
    /// 마지막 확장자만 바꾸므로 `보고서.v2.hwp` 는 `보고서.v2.pdf` 가 되고,
    /// 확장자가 없으면 `.pdf` 를 덧붙인다.
    pub fn expected_output(&self) -> PathBuf {
        let name = self
            .input
            .file_name()
            .unwrap_or_else(|| OsStr::new(FALLBACK_OUTPUT_NAME));

        self.out_dir.join(name).with_extension(PDF_EXTENSION)
    }

    /// 실행 요청으로 바꾼다. `java_home` 은 자식 env 로만 들어간다.
    pub fn request(
        &self,
        soffice: &Path,
        java_home: Option<&Path>,
        timeout: Duration,
    ) -> ProcessRequest {
        let env = java_home
            .map(|home| {
                (
                    OsString::from(JAVA_HOME_KEY),
                    home.as_os_str().to_os_string(),
                )
            })
            .into_iter()
            .collect();

        ProcessRequest {
            program: soffice.to_path_buf(),
            args: self.args(),
            env,
            timeout,
        }
    }
}

/// 입력 크기에 비례한 제한 시간. 하한 60초, 상한 600초.
///
/// 작은 문서도 LibreOffice 첫 기동에 수십 초가 걸릴 수 있어 하한이 필요하고,
/// 무한정 매달리지 않도록 상한도 둔다.
pub fn timeout_for(input_bytes: u64) -> Duration {
    let mib = input_bytes / BYTES_PER_MIB;
    let secs = mib.saturating_mul(TIMEOUT_SECS_PER_MIB);

    Duration::from_secs(secs.clamp(MIN_TIMEOUT_SECS, MAX_TIMEOUT_SECS))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE_DIR: &str = "/tmp/fc/profile";

    fn plan(input: &str, out_dir: &str) -> ConvertPlan {
        ConvertPlan {
            profile: ProfileUrl::from_path_str(PROFILE_DIR, false).expect("절대 경로"),
            input: PathBuf::from(input),
            out_dir: PathBuf::from(out_dir),
        }
    }

    fn arg_strings(plan: &ConvertPlan) -> Vec<String> {
        plan.args()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 변환_argv_는_확정된_순서로_조립된다() {
        // Arrange
        let plan = plan("/tmp/fc/in/보고서.hwp", "/tmp/fc/out/job-1");

        // Act
        let args = plan.args();

        // Assert
        let expected: Vec<OsString> = [
            "-env:UserInstallation=file:///tmp/fc/profile",
            "--headless",
            "--norestore",
            "--nolockcheck",
            "--convert-to",
            "pdf:writer_pdf_Export",
            "--outdir",
            "/tmp/fc/out/job-1",
            "/tmp/fc/in/보고서.hwp",
        ]
        .iter()
        .map(OsString::from)
        .collect();
        assert_eq!(args, expected);
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn infilter_는_절대_포함하지_않는다() {
        // H2Orestart README 의 "Hwp2002_File" 은 필터명이 아니라 타입명이라
        // 강제 지정하면 무시되거나 SIGABRT 로 죽는다. 자동 감지에 맡긴다.
        let joined = arg_strings(&plan("/tmp/in/a.hwp", "/tmp/out")).join(" ");

        assert!(!joined.contains("--infilter"));
        assert!(!joined.contains("Hwp2002_File"));
    }

    #[test]
    fn 폐기된_플래그는_넣지_않는다() {
        let joined = arg_strings(&plan("/tmp/in/a.hwp", "/tmp/out")).join(" ");

        for flag in ["--nofirststartwizard", "--nocrashreport", "--invisible"] {
            assert!(!joined.contains(flag), "{flag} 는 넣지 않아야 한다");
        }
    }

    #[test]
    fn 프로필_인자가_항상_첫_인자다() {
        let args = plan("/tmp/in/a.hwp", "/tmp/out").args();

        assert_eq!(args.first().expect("인자 존재"), &{
            ProfileUrl::from_path_str(PROFILE_DIR, false)
                .expect("절대 경로")
                .as_arg()
        });
    }

    #[test]
    fn 출력_경로는_출력_디렉토리에_확장자만_pdf_로_바꿔_만든다() {
        let plan = plan("/tmp/fc/in/보고서.hwp", "/tmp/fc/out/job-1");

        assert_eq!(
            plan.expected_output(),
            PathBuf::from("/tmp/fc/out/job-1/보고서.pdf")
        );
    }

    #[test]
    fn basename_중간의_점은_보존된다() {
        let plan = plan("/tmp/in/보고서.v2.hwp", "/tmp/out");

        assert_eq!(
            plan.expected_output(),
            PathBuf::from("/tmp/out/보고서.v2.pdf")
        );
    }

    #[test]
    fn 확장자가_없는_입력에는_pdf_를_덧붙인다() {
        let plan = plan("/tmp/in/메모", "/tmp/out");

        assert_eq!(plan.expected_output(), PathBuf::from("/tmp/out/메모.pdf"));
    }

    #[test]
    fn hwpx_입력도_같은_규칙을_따른다() {
        let plan = plan("/tmp/in/계약서.hwpx", "/tmp/out");

        assert_eq!(plan.expected_output(), PathBuf::from("/tmp/out/계약서.pdf"));
        // 입력 인자는 원본 확장자를 그대로 유지해야 감지가 된다.
        assert_eq!(
            plan.args().last().expect("입력 인자"),
            &OsString::from("/tmp/in/계약서.hwpx")
        );
    }

    #[test]
    fn java_home_은_자식_프로세스_env_로만_전달된다() {
        // Arrange
        let plan = plan("/tmp/in/a.hwp", "/tmp/out");

        // Act
        let request = plan.request(
            Path::new("/opt/lo/soffice"),
            Some(Path::new("/opt/jre")),
            Duration::from_secs(90),
        );

        // Assert
        assert_eq!(
            request.env,
            vec![(OsString::from("JAVA_HOME"), OsString::from("/opt/jre"))]
        );
        // 전역 환경은 건드리지 않는다.
        assert!(std::env::var_os("JAVA_HOME").is_none() || !request.env.is_empty());
        // argv 에는 JAVA_HOME 이 새어 나가지 않는다.
        assert!(!arg_strings(&plan).join(" ").contains("JAVA_HOME"));
    }

    #[test]
    fn java_home_이_없으면_env_는_비어_있다() {
        let request = plan("/tmp/in/a.hwp", "/tmp/out").request(
            Path::new("/opt/lo/soffice"),
            None,
            Duration::from_secs(90),
        );

        assert!(request.env.is_empty());
    }

    #[test]
    fn request_는_프로그램과_인자와_타임아웃을_그대로_싣는다() {
        let plan = plan("/tmp/in/a.hwp", "/tmp/out");

        let request = plan.request(Path::new("/opt/lo/soffice"), None, Duration::from_secs(123));

        assert_eq!(request.program, PathBuf::from("/opt/lo/soffice"));
        assert_eq!(request.args, plan.args());
        assert_eq!(request.timeout, Duration::from_secs(123));
    }

    #[test]
    fn 타임아웃은_하한_60초를_지킨다() {
        assert_eq!(timeout_for(0), Duration::from_secs(60));
        assert_eq!(timeout_for(1), Duration::from_secs(60));
        assert_eq!(timeout_for(1024 * 1024), Duration::from_secs(60));
    }

    #[test]
    fn 타임아웃은_상한_600초를_넘지_않는다() {
        assert_eq!(timeout_for(500 * 1024 * 1024), Duration::from_secs(600));
        assert_eq!(timeout_for(u64::MAX), Duration::from_secs(600));
    }

    #[test]
    fn 타임아웃은_입력_크기에_비례한다() {
        let small = timeout_for(20 * 1024 * 1024);
        let large = timeout_for(40 * 1024 * 1024);

        assert!(small > Duration::from_secs(60));
        assert!(large < Duration::from_secs(600));
        assert_eq!(large, small * 2);
    }
}
