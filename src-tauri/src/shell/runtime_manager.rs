//! 런타임(LibreOffice·JRE·H2Orestart) 확보와 변환 실행을 한 곳에서 조율한다.
//!
//! soffice 호출은 전부 이 매니저를 통해 직렬화된다 — 같은 사용자 프로필로 병렬 실행하면
//! 한쪽이 경고 한 줄 없이 산출물 없이 끝난다.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::core::fs_port::FileSystem;
use crate::core::hwp::inspect::preflight_file;
use crate::core::hwp::message::reject_message;
use crate::core::hwp::preflight::Preflight;
use crate::core::runtime::assets::{
    asset_file_name, h2orestart_asset, jre_asset, libreoffice_asset, AssetSpec, Platform,
};
use crate::core::runtime::download::{DownloadProgress, Downloader, ProgressThrottle};
use crate::core::runtime::installer::{
    bundled_extension_dir, jre_dir_name, resolve_java_home, InstallError, ToolInstaller,
};
use crate::core::runtime::plan::{
    is_stale_lock_error, managed_install_root, parse_unopkg_list, profile_lock_file,
    resolve_install_plan, unopkg_add_args, unopkg_list_args, ExtensionState, ExtensionStrategy,
    InstallStep, InstalledLibreOffice, RuntimeStatus,
};
use crate::core::soffice::detect::{detect, unopkg_next_to, SofficeInfo};
use crate::core::soffice::invoke::{timeout_for, ConvertPlan};
use crate::core::soffice::outcome::{
    failure_message, judge, ConvertFailure, ConvertOutcome, JudgeInput,
};
use crate::core::soffice::probe::SofficeProbe;
use crate::core::soffice::profile::ProfileUrl;
use crate::core::soffice::runner::{ProcessRunner, Termination};

const UNOPKG_TIMEOUT: Duration = Duration::from_secs(300);
const PDF_MAGIC_LEN: usize = 5;

/// 앱이 관리하는 런타임의 디렉토리 배치.
#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub root: PathBuf,
    pub libreoffice: PathBuf,
    pub jre: PathBuf,
    pub extension: PathBuf,
    pub downloads: PathBuf,
    pub profile: PathBuf,
    pub work: PathBuf,
}

impl RuntimePaths {
    /// Windows 로밍 프로필은 거부된다 — 수백 MB 를 로밍시키면 로그인이 망가진다.
    pub fn new(app_data_dir: &Path, platform: Platform) -> Result<Self, String> {
        let root = managed_install_root(platform, app_data_dir).map_err(|e| e.to_string())?;

        Ok(Self {
            libreoffice: root.join("libreoffice"),
            jre: root.join("jre"),
            extension: root.join("extension"),
            downloads: root.join("downloads"),
            profile: root.join("profile"),
            work: root.join("work"),
            root,
        })
    }

    fn profile_url(&self) -> Result<ProfileUrl, String> {
        ProfileUrl::from_dir(&self.profile).map_err(|e| e.to_string())
    }
}

/// 설치 진행 상황. 프론트는 이 이벤트로 진행 막대를 그린다.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InstallEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        step: String,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        step: String,
        received: u64,
        total: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    StepDone {
        step: String,
    },
    Finished,
    #[serde(rename_all = "camelCase")]
    Failed {
        message: String,
    },
}

pub struct RuntimeManager {
    probe: Arc<dyn SofficeProbe>,
    runner: Arc<dyn ProcessRunner>,
    fs: Arc<dyn FileSystem>,
    downloader: Arc<dyn Downloader>,
    installer: Arc<dyn ToolInstaller>,
    paths: RuntimePaths,
    platform: Platform,
    cached: RwLock<Option<RuntimeStatus>>,
    /// soffice·unopkg 호출 직렬화용. 프로필 하나당 하나.
    profile_lock: Mutex<()>,
}

impl RuntimeManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        probe: Arc<dyn SofficeProbe>,
        runner: Arc<dyn ProcessRunner>,
        fs: Arc<dyn FileSystem>,
        downloader: Arc<dyn Downloader>,
        installer: Arc<dyn ToolInstaller>,
        paths: RuntimePaths,
        platform: Platform,
    ) -> Self {
        Self {
            probe,
            runner,
            fs,
            downloader,
            installer,
            paths,
            platform,
            cached: RwLock::new(None),
            profile_lock: Mutex::new(()),
        }
    }

    pub fn paths(&self) -> &RuntimePaths {
        &self.paths
    }

    /// 지금 무엇이 준비돼 있는지. `refresh` 가 false 면 캐시를 그대로 쓴다.
    pub fn status(&self, refresh: bool) -> Result<RuntimeStatus, String> {
        if !refresh {
            if let Some(cached) = self.cached.read().ok().and_then(|c| c.clone()) {
                return Ok(cached);
            }
        }

        let status = self.probe_status()?;
        if let Ok(mut slot) = self.cached.write() {
            *slot = Some(status.clone());
        }

        Ok(status)
    }

    /// 현재 채택된 soffice (없으면 None).
    pub fn soffice(&self) -> Result<Option<SofficeInfo>, String> {
        let profile = self.paths.profile_url()?;
        let _guard = self.lock_profile();

        Ok(detect(self.probe.as_ref(), self.runner.as_ref(), &profile))
    }

    fn probe_status(&self) -> Result<RuntimeStatus, String> {
        let soffice = self.soffice()?;
        let java_home = self.find_java_home();

        let extension = match &soffice {
            Some(info) => self.query_extension(&info.exe)?,
            None => ExtensionState::Unknown,
        };

        // JRE 없이 초기화된 프로필은 이후 JAVA_HOME 을 줘도 계속 실패한다 — 지워야 산다.
        let profile_poisoned = java_home.is_none() && self.fs.is_dir(&self.paths.profile);

        Ok(RuntimeStatus {
            libreoffice: soffice.map(|info| InstalledLibreOffice {
                version: info.version,
                managed: info.exe.starts_with(&self.paths.libreoffice),
            }),
            java_home,
            extension,
            profile_poisoned,
        })
    }

    fn find_java_home(&self) -> Option<PathBuf> {
        resolve_java_home(&self.paths.jre, self.platform.os, self.fs.as_ref())
    }

    fn query_extension(&self, soffice: &Path) -> Result<ExtensionState, String> {
        let profile = self.paths.profile_url()?;
        let unopkg = unopkg_next_to(soffice);
        if !self.fs.is_file(&unopkg) {
            return Ok(ExtensionState::Unknown);
        }

        let output = self.run_with_profile(crate::core::soffice::runner::ProcessRequest {
            program: unopkg,
            args: unopkg_list_args(&profile),
            env: self.child_env(),
            timeout: UNOPKG_TIMEOUT,
        });

        match output {
            Ok(output) => Ok(parse_unopkg_list(&output.stdout)),
            Err(_) => Ok(ExtensionState::Unknown),
        }
    }

    /// 자식 프로세스에만 JAVA_HOME 을 준다 — 전역 env 는 건드리지 않는다.
    fn child_env(&self) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        match self.find_java_home() {
            Some(java_home) => vec![(
                std::ffi::OsString::from("JAVA_HOME"),
                java_home.into_os_string(),
            )],
            None => Vec::new(),
        }
    }

    /// 프로필을 쓰는 실행. 남은 잠금 파일 때문에 시작조차 못 하면 한 번 치우고 재시도한다.
    fn run_with_profile(
        &self,
        request: crate::core::soffice::runner::ProcessRequest,
    ) -> Result<crate::core::soffice::runner::ProcessOutput, String> {
        let _guard = self.lock_profile();

        let first = self
            .runner
            .run(&request)
            .map_err(|error| error.to_string())?;
        if !is_stale_lock_error(&first.stderr) {
            return Ok(first);
        }

        // 비정상 종료로 남은 찌꺼기다 — 전용 프로필이라 다른 인스턴스일 리 없다.
        let _ = self.fs.remove_file(&profile_lock_file(&self.paths.profile));

        self.runner.run(&request).map_err(|error| error.to_string())
    }

    fn lock_profile(&self) -> std::sync::MutexGuard<'_, ()> {
        self.profile_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 부족한 것을 계획 순서대로 채운다. 이미 준비된 것은 건너뛴다.
    pub fn install(
        &self,
        on_event: &mut dyn FnMut(InstallEvent),
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<RuntimeStatus, String> {
        let status = self.status(true)?;
        let steps = resolve_install_plan(&status, self.platform);

        for step in steps {
            let label = step_label(&step);
            on_event(InstallEvent::Started {
                step: label.to_string(),
            });

            self.run_step(&step, label, on_event, is_cancelled)
                .inspect_err(|error| {
                    on_event(InstallEvent::Failed {
                        message: error.clone(),
                    });
                })?;

            on_event(InstallEvent::StepDone {
                step: label.to_string(),
            });
        }

        on_event(InstallEvent::Finished);
        self.status(true)
    }

    fn run_step(
        &self,
        step: &InstallStep,
        label: &str,
        on_event: &mut dyn FnMut(InstallEvent),
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        match step {
            InstallStep::DownloadLibreOffice(spec) => {
                self.download(spec, label, on_event, is_cancelled)
            }
            InstallStep::DownloadJre(spec) => self.download(spec, label, on_event, is_cancelled),
            InstallStep::DownloadExtension(spec) => {
                self.download(spec, label, on_event, is_cancelled)
            }
            InstallStep::InstallLibreOffice => self
                .installer
                .install_libreoffice(
                    &self.archive_path(&libreoffice_asset(self.platform)),
                    &self.paths.libreoffice,
                )
                .map(|_| ())
                .map_err(stringify),
            InstallStep::InstallJre => self
                .installer
                .install_jre(
                    &self.archive_path(&jre_asset(self.platform)),
                    &self.paths.jre,
                )
                .map(|_| ())
                .map_err(stringify),
            InstallStep::InstallExtension(strategy) => match strategy {
                ExtensionStrategy::BundledDir => self.install_extension_bundled(),
                ExtensionStrategy::UserProfile => self.install_extension_via_unopkg(),
            },
            InstallStep::ResetProfile => {
                let _ = self.fs.remove_dir_all(&self.paths.profile);
                Ok(())
            }
            InstallStep::VerifyExtension => self.verify_extension(),
        }
    }

    /// 내려받은 파일은 **URL 의 파일명 그대로** 저장한다 — 확장자를 잃으면
    /// `.tar.gz` 를 zip 으로 풀려다 실패한다 (실제로 여기서 설치가 깨졌다).
    fn archive_path(&self, spec: &AssetSpec) -> PathBuf {
        self.paths.downloads.join(asset_file_name(spec))
    }

    fn download(
        &self,
        spec: &AssetSpec,
        label: &str,
        on_event: &mut dyn FnMut(InstallEvent),
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        self.fs
            .create_dir_all(&self.paths.downloads)
            .map_err(|e| e.to_string())?;

        let dest = self.archive_path(spec);
        let started = Instant::now();
        let mut throttle = ProgressThrottle::new();
        let step = label.to_string();

        self.downloader
            .download_verified(
                spec,
                &dest,
                &mut |DownloadProgress { received, total }| {
                    // 청크마다 이벤트를 쏘면 웹뷰가 밀린다.
                    if throttle.should_report(received, total, started.elapsed().as_millis() as u64)
                    {
                        on_event(InstallEvent::Progress {
                            step: step.clone(),
                            received,
                            total,
                        });
                    }
                },
                is_cancelled,
            )
            .map_err(|error| error.to_string())
    }

    /// 앱이 설치한 LibreOffice 에는 번들 확장 디렉토리에 직접 풀고 한 번 기동해 등록시킨다.
    ///
    /// macOS 에서 `unopkg add` 는 UNO 파이프에 붙지 못해
    /// `NoConnectException` 으로 실패한다 — 이 경로가 사실상 유일한 방법이다.
    fn install_extension_bundled(&self) -> Result<(), String> {
        let soffice = self
            .soffice()?
            .ok_or_else(|| "LibreOffice 를 찾지 못했습니다".to_string())?;
        let target = bundled_extension_dir(&soffice.exe, self.platform.os)
            .ok_or_else(|| "확장을 넣을 위치를 찾지 못했습니다".to_string())?;

        // 이전 버전이 남아 있으면 섞인다.
        let _ = self.fs.remove_dir_all(&target);
        self.installer
            .unpack_oxt(&self.archive_path(&h2orestart_asset()), &target)
            .map_err(stringify)?;

        // 번들 확장은 기동할 때 스캔돼 사용자 프로필에 등록된다.
        self.warm_up_profile(&soffice.exe)
    }

    /// 확장 등록을 트리거하기 위한 1회 기동.
    fn warm_up_profile(&self, soffice: &Path) -> Result<(), String> {
        let profile = self.paths.profile_url()?;

        self.run_with_profile(crate::core::soffice::runner::ProcessRequest {
            program: soffice.to_path_buf(),
            args: vec![
                profile.as_arg(),
                std::ffi::OsString::from("--headless"),
                std::ffi::OsString::from("--terminate_after_init"),
            ],
            env: self.child_env(),
            timeout: UNOPKG_TIMEOUT,
        })?;

        // 기동이 비정상 종료하면 잠금이 남아 다음 호출을 막는다.
        let _ = self.fs.remove_file(&profile_lock_file(&self.paths.profile));

        Ok(())
    }

    fn install_extension_via_unopkg(&self) -> Result<(), String> {
        let soffice = self
            .soffice()?
            .ok_or_else(|| "LibreOffice 를 찾지 못했습니다".to_string())?;
        let profile = self.paths.profile_url()?;
        let unopkg = unopkg_next_to(&soffice.exe);
        let oxt = self.archive_path(&h2orestart_asset());

        let output = self.run_with_profile(crate::core::soffice::runner::ProcessRequest {
            program: unopkg,
            args: unopkg_add_args(&oxt, &profile),
            env: self.child_env(),
            timeout: UNOPKG_TIMEOUT,
        })?;

        match output.termination {
            Termination::Code(0) => Ok(()),
            _ => Err(format!(
                "확장 설치에 실패했습니다: {}",
                output.stderr.lines().next().unwrap_or("").trim()
            )),
        }
    }

    fn verify_extension(&self) -> Result<(), String> {
        let soffice = self
            .soffice()?
            .ok_or_else(|| "LibreOffice 를 찾지 못했습니다".to_string())?;

        match self.query_extension(&soffice.exe)? {
            ExtensionState::Registered { version } if version == h2orestart_version() => Ok(()),
            ExtensionState::Registered { version } => {
                Err(format!("설치된 확장 버전이 다릅니다 ({version})"))
            }
            _ => Err("확장이 등록되지 않았습니다".to_string()),
        }
    }

    /// 한 건 변환. 프리플라이트 → soffice → 판정 → 산출물 이동 순서를 지킨다.
    pub fn convert_to_pdf(&self, input: &Path, out_path: &Path) -> Result<(), String> {
        match preflight_file(input).map_err(|e| e.to_string())? {
            Preflight::Reject(reason) => return Err(reject_message(reason).to_string()),
            Preflight::Proceed | Preflight::ProceedWithNote(_) => {}
        }

        let soffice = self
            .soffice()?
            .ok_or_else(|| "LibreOffice 가 준비되지 않았습니다".to_string())?;
        let profile = self.paths.profile_url()?;
        let out_dir = self.unique_work_dir();
        self.fs
            .create_dir_all(&out_dir)
            .map_err(|e| e.to_string())?;

        let plan = ConvertPlan {
            profile,
            input: input.to_path_buf(),
            out_dir: out_dir.clone(),
        };
        let java_home = self.find_java_home();
        let timeout = timeout_for(self.fs.len(input).unwrap_or(0));

        let outcome = {
            let request = plan.request(&soffice.exe, java_home.as_deref(), timeout);
            let output = self.run_with_profile(request)?;

            let produced = plan.expected_output();
            judge(&JudgeInput {
                termination: output.termination,
                stdout: &output.stdout,
                stderr: &output.stderr,
                output_exists: self.fs.is_file(&produced),
                output_len: self.fs.len(&produced).unwrap_or(0),
                output_magic: self.read_magic(&produced),
                trusts_exit_code: soffice.version.trusts_exit_code(),
            })
        };

        let result = self.finish_conversion(outcome, &plan.expected_output(), out_path);
        let _ = self.fs.remove_dir_all(&out_dir);

        result
    }

    fn finish_conversion(
        &self,
        outcome: ConvertOutcome,
        produced: &Path,
        out_path: &Path,
    ) -> Result<(), String> {
        match outcome {
            ConvertOutcome::Failed(failure) => Err(failure_message(&failure)),
            // 산출물이 의심스러워도 사용자에게 넘기되, 호출자가 경고를 띄울 수 있게 성공으로 둔다.
            ConvertOutcome::Ok | ConvertOutcome::SuspectEmpty { .. } => {
                if let Some(parent) = out_path.parent() {
                    self.fs.create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                self.fs.rename(produced, out_path).or_else(|_| {
                    // 다른 볼륨이면 rename 이 실패한다 — 복사로 폴백한다.
                    std::fs::copy(produced, out_path)
                        .map(|_| ())
                        .map_err(|error| failure_message(&ConvertFailure::Other(error.to_string())))
                })
            }
        }
    }

    fn read_magic(&self, path: &Path) -> Option<[u8; PDF_MAGIC_LEN]> {
        let bytes = self.fs.read_prefix(path, PDF_MAGIC_LEN).ok()?;
        bytes.try_into().ok()
    }

    /// 잡마다 고유한 출력 디렉토리 — 같은 basename 두 파일이 말없이 덮어써지는 것을 막는다.
    fn unique_work_dir(&self) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();

        self.paths.work.join(format!("job-{stamp}"))
    }
}

fn h2orestart_version() -> String {
    crate::core::runtime::assets::H2O_VERSION.to_string()
}

fn stringify(error: InstallError) -> String {
    error.to_string()
}

fn step_label(step: &InstallStep) -> &'static str {
    match step {
        InstallStep::DownloadLibreOffice(_) => "LibreOffice 내려받는 중",
        InstallStep::InstallLibreOffice => "LibreOffice 설치 중",
        InstallStep::DownloadJre(_) => "Java 런타임 내려받는 중",
        InstallStep::InstallJre => "Java 런타임 설치 중",
        InstallStep::DownloadExtension(_) => "한글 문서 확장 내려받는 중",
        InstallStep::InstallExtension(_) => "한글 문서 확장 설치 중",
        InstallStep::ResetProfile => "설정 초기화 중",
        InstallStep::VerifyExtension => "설치 확인 중",
    }
}

/// 다운로드 자산 이름 → 파일명 (진행 표시용 라벨과 별개).
pub fn extension_asset_name() -> &'static str {
    let _ = h2orestart_asset();
    "H2Orestart.oxt"
}

/// 압축을 푼 JRE 의 최상위 디렉토리 이름 (설정 화면 표시용).
pub fn jre_folder_name() -> String {
    jre_dir_name()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::runtime::assets::{Arch, Os};

    fn platform(os: Os) -> Platform {
        Platform {
            os,
            arch: Arch::Aarch64,
        }
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 런타임_디렉토리는_모두_루트_아래에_모인다() {
        let paths = RuntimePaths::new(
            Path::new("/Users/kim/Library/Application Support/fc"),
            platform(Os::MacOs),
        )
        .expect("경로 생성");

        for dir in [
            &paths.libreoffice,
            &paths.jre,
            &paths.extension,
            &paths.downloads,
            &paths.profile,
            &paths.work,
        ] {
            assert!(dir.starts_with(&paths.root), "루트 밖: {dir:?}");
        }
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 윈도_로밍_프로필에는_런타임을_두지_않는다() {
        let result = RuntimePaths::new(
            Path::new(r"C:\Users\kim\AppData\Roaming\fc"),
            platform(Os::Windows),
        );

        assert!(result.is_err());
    }

    #[test]
    fn 프로필_url_은_env_인자로_바로_쓸_수_있다() {
        // ProfileUrl 은 호스트 규칙으로 절대 경로를 요구하므로 기준 경로도 호스트 것을 쓴다.
        let base = std::env::temp_dir().join("fc-runtime-paths");
        let paths = RuntimePaths::new(&base, platform(Os::MacOs)).expect("경로 생성");

        let arg = paths.profile_url().expect("URL").as_arg();

        assert!(arg
            .to_string_lossy()
            .starts_with("-env:UserInstallation=file:///"));
    }

    #[test]
    fn 모든_설치_단계에_한국어_라벨이_있다() {
        let spec = h2orestart_asset();
        let steps = [
            InstallStep::DownloadLibreOffice(spec),
            InstallStep::InstallLibreOffice,
            InstallStep::DownloadJre(spec),
            InstallStep::InstallJre,
            InstallStep::DownloadExtension(spec),
            InstallStep::InstallExtension(
                crate::core::runtime::plan::ExtensionStrategy::BundledDir,
            ),
            InstallStep::ResetProfile,
            InstallStep::VerifyExtension,
        ];

        for step in steps {
            let label = step_label(&step);
            assert!(!label.is_empty(), "라벨 없음: {step:?}");
            assert!(label.contains("중"), "진행형이 아님: {label}");
        }
    }

    #[test]
    fn 설치_이벤트는_kind_태그로_직렬화된다() {
        let json = serde_json::to_value(InstallEvent::Progress {
            step: "내려받는 중".to_string(),
            received: 10,
            total: Some(100),
        })
        .expect("직렬화");

        assert_eq!(json["kind"], "progress");
        assert_eq!(json["received"], 10);
    }
}
