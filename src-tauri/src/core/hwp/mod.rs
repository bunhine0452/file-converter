//! 한글 문서(HWP5·HWPX) 프리플라이트와 사용자 대면 메시지.
//!
//! H2Orestart 는 암호 문서를 만나도 예외를 삼키고 **빈 PDF 와 exit 0** 을 낸다.
//! 그래서 변환을 시작하기 전에 우리가 직접 문서 헤더를 읽어 걸러야 한다.

pub mod message;
pub mod preflight;
