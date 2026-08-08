//! 외부 프로세스 실행 포트.
//!
//! LibreOffice·unopkg 는 프로세스 경계 너머에서만 쓴다 (라이선스 경계 유지).
//! 실행 자체를 트레이트로 감싸 argv 조립과 결과 판정을 순수 함수로 테스트한다.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRequest {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// 자식 프로세스에만 적용한다 — 전역 env 를 오염시키지 않는다.
    pub env: Vec<(OsString, OsString)>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    Code(i32),
    /// Unix 시그널 종료 (134=SIGABRT, 139=SIGSEGV).
    Signal(i32),
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub termination: Termination,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("실행 파일을 찾을 수 없습니다: {0}")]
    NotFound(String),
    #[error("프로세스 실행 실패: {0}")]
    Spawn(String),
}

pub trait ProcessRunner: Send + Sync {
    fn run(&self, request: &ProcessRequest) -> Result<ProcessOutput, RunError>;
}

/// 실제 프로세스 실행. 타임아웃이 지나면 강제 종료한다.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealRunner;

impl ProcessRunner for RealRunner {
    fn run(&self, request: &ProcessRequest) -> Result<ProcessOutput, RunError> {
        use std::process::{Command, Stdio};

        let mut command = Command::new(&request.program);
        command
            .args(&request.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &request.env {
            command.env(key, value);
        }

        let mut child = command.spawn().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                RunError::NotFound(request.program.display().to_string())
            } else {
                RunError::Spawn(error.to_string())
            }
        })?;

        let deadline = std::time::Instant::now() + request.timeout;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Ok(ProcessOutput {
                            termination: Termination::TimedOut,
                            stdout: String::new(),
                            stderr: String::new(),
                        });
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(error) => return Err(RunError::Spawn(error.to_string())),
            }
        }

        let output = child
            .wait_with_output()
            .map_err(|error| RunError::Spawn(error.to_string()))?;

        Ok(ProcessOutput {
            termination: termination_of(&output.status),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(unix)]
fn termination_of(status: &std::process::ExitStatus) -> Termination {
    use std::os::unix::process::ExitStatusExt;

    match (status.code(), status.signal()) {
        (Some(code), _) => Termination::Code(code),
        (None, Some(signal)) => Termination::Signal(signal),
        _ => Termination::Code(-1),
    }
}

#[cfg(not(unix))]
fn termination_of(status: &std::process::ExitStatus) -> Termination {
    Termination::Code(status.code().unwrap_or(-1))
}

/// 테스트용 실행기 — 프로그램 경로별로 미리 정해둔 출력을 돌려주고 호출을 기록한다.
#[cfg(test)]
pub mod fake {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    /// 실제 도구의 부작용을 흉내내는 훅. 요청을 받아야 `--outdir` 같은 인자를 볼 수 있다.
    type Effect = Box<dyn Fn(&ProcessRequest) + Send + Sync>;
    /// 인자에 따라 다른 출력을 내는 응답기 (같은 프로그램을 스코프만 바꿔 부르는 경우).
    type Responder = Box<dyn Fn(&ProcessRequest) -> ProcessOutput + Send + Sync>;

    #[derive(Default)]
    pub struct FakeRunner {
        responses: Mutex<BTreeMap<PathBuf, ProcessOutput>>,
        responders: Mutex<BTreeMap<PathBuf, Responder>>,
        default_response: Mutex<Option<ProcessOutput>>,
        effects: Mutex<BTreeMap<PathBuf, Effect>>,
        calls: Mutex<Vec<ProcessRequest>>,
    }

    impl FakeRunner {
        pub fn new() -> Self {
            Self::default()
        }

        /// 이 프로그램 경로로 실행되면 이 출력을 돌려준다.
        pub fn responding(self, program: impl Into<PathBuf>, output: ProcessOutput) -> Self {
            self.responses
                .lock()
                .unwrap()
                .insert(program.into(), output);
            self
        }

        /// 인자를 보고 출력을 고른다 — 고정 응답보다 우선한다.
        pub fn responding_with(
            self,
            program: impl Into<PathBuf>,
            responder: impl Fn(&ProcessRequest) -> ProcessOutput + Send + Sync + 'static,
        ) -> Self {
            self.responders
                .lock()
                .unwrap()
                .insert(program.into(), Box::new(responder));
            self
        }

        /// 매핑되지 않은 모든 프로그램에 대한 기본 출력.
        pub fn default_response(self, output: ProcessOutput) -> Self {
            *self.default_response.lock().unwrap() = Some(output);
            self
        }

        /// 이 프로그램이 실행되면 부작용을 일으킨다 — 실제 도구가 파일을 만드는 것을 흉내낸다.
        pub fn on_run(
            self,
            program: impl Into<PathBuf>,
            effect: impl Fn(&ProcessRequest) + Send + Sync + 'static,
        ) -> Self {
            self.effects
                .lock()
                .unwrap()
                .insert(program.into(), Box::new(effect));
            self
        }

        pub fn calls(&self) -> Vec<ProcessRequest> {
            self.calls.lock().unwrap().clone()
        }

        pub fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    impl ProcessRunner for FakeRunner {
        fn run(&self, request: &ProcessRequest) -> Result<ProcessOutput, RunError> {
            self.calls.lock().unwrap().push(request.clone());

            if let Some(effect) = self.effects.lock().unwrap().get(&request.program) {
                effect(request);
            }

            if let Some(responder) = self.responders.lock().unwrap().get(&request.program) {
                return Ok(responder(request));
            }
            if let Some(output) = self.responses.lock().unwrap().get(&request.program) {
                return Ok(output.clone());
            }
            if let Some(output) = self.default_response.lock().unwrap().clone() {
                return Ok(output);
            }

            Err(RunError::NotFound(request.program.display().to_string()))
        }
    }

    /// 테스트에서 자주 쓰는 성공 출력 헬퍼.
    pub fn ok_output(stdout: &str) -> ProcessOutput {
        ProcessOutput {
            termination: Termination::Code(0),
            stdout: stdout.to_string(),
            stderr: String::new(),
        }
    }
}
