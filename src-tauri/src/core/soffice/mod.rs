//! LibreOffice(soffice) 를 외부 프로세스로만 다루는 계층.
//!
//! 라이선스 경계: LibreOffice·H2Orestart 코드를 링크하지 않는다 — argv 조립과
//! 출력 해석만 하고 실행은 [`runner::ProcessRunner`] 뒤에서 일어난다.

pub mod detect;
pub mod invoke;
pub mod outcome;
pub mod probe;
pub mod profile;
pub mod runner;
pub mod version;
