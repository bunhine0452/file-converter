//! 런타임 자산(LibreOffice·JRE·H2Orestart) 온디맨드 확보.
//!
//! 허용된 네트워크 사용은 도구 바이너리 다운로드뿐이며, 반드시 해시를 검증한다.
//! 사용자 파일은 어떤 경우에도 네트워크로 나가지 않는다.

pub mod assets;
pub mod download;
pub mod installer;
pub mod plan;
pub mod real_installer;
