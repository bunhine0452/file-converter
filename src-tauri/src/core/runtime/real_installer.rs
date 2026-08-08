//! [`ToolInstaller`] 의 실제 구현.
//!
//! dmg 마운트·번들 복사·msi 관리 설치는 OS 도구를 **외부 프로세스로** 부르고,
//! zip 계열만 크레이트로 푼다. 경로 규칙과 argv 는 [`super::installer`] 의 순수 함수를 쓴다.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::core::fs_port::FileSystem;
use crate::core::runtime::assets::Os;
use crate::core::runtime::installer::{
    bundled_extension_dir, ditto_args, hdiutil_attach_args, hdiutil_detach_args,
    msiexec_admin_args, parse_hdiutil_mount_point, resolve_java_home, soffice_candidates,
    xattr_clear_quarantine_args, InstallError, ToolInstaller,
};
use crate::core::soffice::runner::{ProcessRequest, ProcessRunner, Termination};

/// 대용량 아카이브를 푸는 데 걸리는 시간. 300MB dmg 복사가 여기 들어간다.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(900);
/// 마운트/해제처럼 짧게 끝나야 하는 작업.
const MOUNT_TIMEOUT: Duration = Duration::from_secs(120);

pub struct RealInstaller {
    runner: Arc<dyn ProcessRunner>,
    fs: Arc<dyn FileSystem>,
    os: Os,
}

impl RealInstaller {
    pub fn new(runner: Arc<dyn ProcessRunner>, fs: Arc<dyn FileSystem>, os: Os) -> Self {
        Self { runner, fs, os }
    }

    fn run(
        &self,
        program: &str,
        args: Vec<std::ffi::OsString>,
        timeout: Duration,
    ) -> Result<String, InstallError> {
        let output = self
            .runner
            .run(&ProcessRequest {
                program: PathBuf::from(program),
                args,
                env: Vec::new(),
                timeout,
            })
            .map_err(|error| InstallError::Tool(error.to_string()))?;

        match output.termination {
            Termination::Code(0) => Ok(output.stdout),
            Termination::Code(code) => Err(InstallError::Tool(format!(
                "{program} 이(가) 종료 코드 {code} 로 끝났습니다: {}",
                first_line(&output.stderr)
            ))),
            Termination::Signal(signal) => Err(InstallError::Tool(format!(
                "{program} 이(가) 시그널 {signal} 로 종료됐습니다"
            ))),
            Termination::TimedOut => Err(InstallError::Tool(format!(
                "{program} 이(가) 제한 시간을 넘겼습니다"
            ))),
        }
    }

    /// dmg 를 붙여 `.app` 을 복사하고 반드시 다시 뗀다.
    fn install_dmg_bundle(&self, dmg: &Path, dest: &Path) -> Result<PathBuf, InstallError> {
        let plist = self.run("hdiutil", hdiutil_attach_args(dmg), MOUNT_TIMEOUT)?;
        let mount_point = parse_hdiutil_mount_point(&plist)
            .ok_or_else(|| InstallError::Tool("dmg 마운트 지점을 읽지 못했습니다".to_string()))?;

        let result = self.copy_bundle_from(&mount_point, dest);

        // 복사가 실패해도 마운트는 반드시 정리한다.
        if let Err(error) = self.run("hdiutil", hdiutil_detach_args(&mount_point), MOUNT_TIMEOUT) {
            eprintln!("dmg 마운트 해제 실패(무시하고 진행): {error}");
        }

        result
    }

    fn copy_bundle_from(&self, mount_point: &Path, dest: &Path) -> Result<PathBuf, InstallError> {
        let source = mount_point.join("LibreOffice.app");
        if !self.fs.is_dir(&source) {
            return Err(InstallError::NotFoundAfterInstall(
                "LibreOffice.app".to_string(),
            ));
        }

        self.fs
            .create_dir_all(dest)
            .map_err(|error| InstallError::Extract(error.to_string()))?;
        let target = dest.join("LibreOffice.app");
        // 이전 설치가 남아 있으면 ditto 가 섞어버린다 — 먼저 비운다.
        let _ = self.fs.remove_dir_all(&target);

        self.run("ditto", ditto_args(&source, &target), INSTALL_TIMEOUT)?;
        // 격리 속성이 남으면 Gatekeeper 가 실행을 막는다. 실패해도 치명적이지는 않다.
        if let Err(error) = self.run("xattr", xattr_clear_quarantine_args(&target), MOUNT_TIMEOUT) {
            eprintln!("격리 속성 제거 실패(무시하고 진행): {error}");
        }

        Ok(dest.to_path_buf())
    }

    fn extract_archive(&self, archive: &Path, dest: &Path) -> Result<(), InstallError> {
        self.fs
            .create_dir_all(dest)
            .map_err(|error| InstallError::Extract(error.to_string()))?;

        if is_tarball(archive) {
            // tar 는 macOS/Windows 10+ 에 기본 탑재돼 있고, 실행 권한을 그대로 보존한다.
            self.run(
                "tar",
                vec![
                    std::ffi::OsString::from("-xzf"),
                    archive.as_os_str().to_os_string(),
                    std::ffi::OsString::from("-C"),
                    dest.as_os_str().to_os_string(),
                ],
                INSTALL_TIMEOUT,
            )?;
            return Ok(());
        }

        extract_zip(archive, dest)
    }
}

impl ToolInstaller for RealInstaller {
    fn install_libreoffice(&self, archive: &Path, dest: &Path) -> Result<PathBuf, InstallError> {
        let root = match self.os {
            Os::MacOs => self.install_dmg_bundle(archive, dest)?,
            Os::Windows => {
                self.fs
                    .create_dir_all(dest)
                    .map_err(|error| InstallError::Extract(error.to_string()))?;
                self.run(
                    "msiexec",
                    msiexec_admin_args(archive, dest),
                    INSTALL_TIMEOUT,
                )?;
                dest.to_path_buf()
            }
        };

        // 설치가 끝났다고 믿지 말고 실행 파일이 실제로 있는지 확인한다.
        soffice_candidates(&root, self.os)
            .into_iter()
            .find(|candidate| self.fs.is_file(candidate))
            .ok_or_else(|| InstallError::NotFoundAfterInstall("soffice".to_string()))?;

        Ok(root)
    }

    fn install_jre(&self, archive: &Path, dest: &Path) -> Result<PathBuf, InstallError> {
        self.extract_archive(archive, dest)?;

        resolve_java_home(dest, self.os, self.fs.as_ref())
            .ok_or_else(|| InstallError::NotFoundAfterInstall("JAVA_HOME".to_string()))
    }

    fn unpack_oxt(&self, oxt: &Path, dest: &Path) -> Result<(), InstallError> {
        self.fs
            .create_dir_all(dest)
            .map_err(|error| InstallError::Extract(error.to_string()))?;

        extract_zip(oxt, dest)
    }
}

/// 확장을 풀어 넣을 위치. 설치 트리를 알 수 없으면 `None`.
pub fn extension_target_dir(soffice: &Path, os: Os) -> Option<PathBuf> {
    bundled_extension_dir(soffice, os)
}

fn is_tarball(archive: &Path) -> bool {
    let name = archive.to_string_lossy().to_ascii_lowercase();

    name.ends_with(".tar.gz") || name.ends_with(".tgz")
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), InstallError> {
    let file =
        std::fs::File::open(archive).map_err(|error| InstallError::Extract(error.to_string()))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|error| InstallError::Extract(error.to_string()))?;

    zip.extract(dest)
        .map_err(|error| InstallError::Extract(error.to_string()))
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("").trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::fs_port::fake::FakeFs;
    use crate::core::soffice::runner::fake::{ok_output, FakeRunner};
    use crate::core::soffice::runner::ProcessOutput;

    const MOUNT: &str = "/Volumes/LibreOffice";
    const DEST: &str = "/data/runtime/lo";

    fn attach_plist() -> String {
        format!(
            "<plist><dict><key>system-entities</key><array><dict>\
             <key>mount-point</key><string>{MOUNT}</string></dict></array></dict></plist>"
        )
    }

    /// dmg 설치가 성공하는 세계 — 마운트 지점에 번들이 있고, 복사 후 실행 파일이 생긴다.
    fn macos_success() -> (RealInstaller, Arc<FakeRunner>) {
        let fs = Arc::new(FakeFs::new().with_dir(format!("{MOUNT}/LibreOffice.app")));
        let created = Arc::clone(&fs);
        let runner = Arc::new(
            FakeRunner::new()
                .responding("hdiutil", ok_output(&attach_plist()))
                // 실제 ditto 는 번들을 복사해 실행 파일을 만들어낸다.
                .on_run("ditto", move |_| {
                    created.add_file(
                        format!("{DEST}/LibreOffice.app/Contents/MacOS/soffice"),
                        b"bin".to_vec(),
                    )
                })
                .default_response(ok_output("")),
        );

        (RealInstaller::new(runner.clone(), fs, Os::MacOs), runner)
    }

    fn programs(runner: &FakeRunner) -> Vec<String> {
        runner
            .calls()
            .iter()
            .map(|call| call.program.to_string_lossy().into_owned())
            .collect()
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 맥에서_dmg_를_붙여_복사하고_다시_뗀다() {
        let (installer, runner) = macos_success();

        let root = installer
            .install_libreoffice(Path::new("/tmp/lo.dmg"), Path::new(DEST))
            .expect("설치 성공");

        assert_eq!(root, PathBuf::from(DEST));
        let called = programs(&runner);
        assert_eq!(called.iter().filter(|p| *p == "hdiutil").count(), 2);
        assert!(called.contains(&"ditto".to_string()));
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 복사에_실패해도_마운트는_반드시_해제한다() {
        // Arrange — 마운트 지점에 번들이 없다.
        let runner = Arc::new(
            FakeRunner::new()
                .responding("hdiutil", ok_output(&attach_plist()))
                .default_response(ok_output("")),
        );
        let installer = RealInstaller::new(runner.clone(), Arc::new(FakeFs::new()), Os::MacOs);

        // Act
        let result = installer.install_libreoffice(Path::new("/tmp/lo.dmg"), Path::new(DEST));

        // Assert
        assert!(matches!(result, Err(InstallError::NotFoundAfterInstall(_))));
        let detached = runner.calls().iter().any(|call| {
            call.args
                .iter()
                .any(|arg| arg.to_string_lossy() == "detach")
        });
        assert!(detached, "마운트가 남으면 다음 설치가 막힌다");
    }

    #[test]
    fn 마운트_지점을_못_읽으면_복사를_시도하지_않는다() {
        let runner = Arc::new(
            FakeRunner::new()
                .responding("hdiutil", ok_output("<plist></plist>"))
                .default_response(ok_output("")),
        );
        let installer = RealInstaller::new(runner.clone(), Arc::new(FakeFs::new()), Os::MacOs);

        let result = installer.install_libreoffice(Path::new("/tmp/lo.dmg"), Path::new(DEST));

        assert!(matches!(result, Err(InstallError::Tool(_))));
        assert!(!programs(&runner).contains(&"ditto".to_string()));
    }

    #[test]
    fn 설치_후_실행_파일이_없으면_실패로_보고한다() {
        let runner = Arc::new(
            FakeRunner::new()
                .responding("hdiutil", ok_output(&attach_plist()))
                .default_response(ok_output("")),
        );
        // 번들은 있지만 복사 결과에 실행 파일이 없다.
        let fs = Arc::new(FakeFs::new().with_dir(format!("{MOUNT}/LibreOffice.app")));
        let installer = RealInstaller::new(runner, fs, Os::MacOs);

        let result = installer.install_libreoffice(Path::new("/tmp/lo.dmg"), Path::new(DEST));

        assert_eq!(
            result,
            Err(InstallError::NotFoundAfterInstall("soffice".to_string()))
        );
    }

    #[test]
    fn 도구가_실패하면_stderr_첫_줄을_에러에_담는다() {
        let runner = Arc::new(FakeRunner::new().default_response(ProcessOutput {
            termination: Termination::Code(1),
            stdout: String::new(),
            stderr: "hdiutil: attach failed - no mountable file systems\n자세한 내용".to_string(),
        }));
        let installer = RealInstaller::new(runner, Arc::new(FakeFs::new()), Os::MacOs);

        let error = installer
            .install_libreoffice(Path::new("/tmp/lo.dmg"), Path::new(DEST))
            .expect_err("실패");

        assert!(
            error.to_string().contains("no mountable file systems"),
            "원인이 사라지면 안 된다: {error}"
        );
    }

    #[test]
    fn 윈도는_msiexec_관리_설치를_쓴다() {
        let runner = Arc::new(FakeRunner::new().default_response(ok_output("")));
        let fs = Arc::new(
            FakeFs::new().with_file(format!("{DEST}/program/soffice.com"), b"bin".to_vec()),
        );
        let installer = RealInstaller::new(runner.clone(), fs, Os::Windows);

        installer
            .install_libreoffice(Path::new("/tmp/lo.msi"), Path::new(DEST))
            .expect("설치 성공");

        assert_eq!(programs(&runner), vec!["msiexec".to_string()]);
    }

    #[test]
    fn tar_gz_는_tar_로_풀고_java_home_을_찾는다() {
        let runner = Arc::new(FakeRunner::new().default_response(ok_output("")));
        let java_home = format!(
            "/data/runtime/jre/{}/Contents/Home",
            crate::core::runtime::installer::jre_dir_name()
        );
        let fs = Arc::new(
            FakeFs::new()
                .with_dir(java_home.clone())
                .with_file(format!("{java_home}/bin/java"), b"bin".to_vec()),
        );
        let installer = RealInstaller::new(runner.clone(), fs, Os::MacOs);

        let found = installer
            .install_jre(Path::new("/tmp/jre.tar.gz"), Path::new("/data/runtime/jre"))
            .expect("설치 성공");

        assert_eq!(found, PathBuf::from(java_home));
        assert_eq!(programs(&runner), vec!["tar".to_string()]);
    }

    #[test]
    fn 빈_디렉토리는_java_home_으로_인정하지_않는다() {
        let runner = Arc::new(FakeRunner::new().default_response(ok_output("")));
        let installer = RealInstaller::new(runner, Arc::new(FakeFs::new()), Os::MacOs);

        let result =
            installer.install_jre(Path::new("/tmp/jre.tar.gz"), Path::new("/data/runtime/jre"));

        assert_eq!(
            result,
            Err(InstallError::NotFoundAfterInstall("JAVA_HOME".to_string()))
        );
    }

    #[test]
    fn 확장자로_tar_와_zip_을_가른다() {
        assert!(is_tarball(Path::new("/tmp/OpenJDK21U-jre.tar.gz")));
        assert!(is_tarball(Path::new("/tmp/a.TGZ")));
        assert!(!is_tarball(Path::new("/tmp/OpenJDK21U-jre.zip")));
        assert!(!is_tarball(Path::new("/tmp/H2Orestart.oxt")));
    }
}
