//! 변환 코어. Tauri 셸과 분리해 순수 로직만 담는다 (단위 테스트 가능해야 한다).

pub mod events;
pub mod file_type;
pub mod fs_port;
pub mod hwp;
pub mod job;
pub mod progress;
pub mod runtime;
pub mod soffice;
