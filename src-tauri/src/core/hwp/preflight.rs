//! HWP/HWPX 프리플라이트 — soffice 를 띄우기 **전에** 문서 헤더를 읽어 변환 불가 문서를 걸러낸다.
//!
//! H2Orestart 의 `impl_import()` 는 `HwpParseException` 을 잡고도 return 하지 않고 계속 진행해
//! 마지막에 무조건 성공을 반환한다. 그 결과 암호가 걸린 HWP 를 넣으면 사용자는 아무 에러 없이
//! **빈 PDF** 를 받는다. 이 모듈은 그 실패를 변환 이전 단계에서 명시적 거부로 바꾼다.

/// FileHeader 스트림에서 우리가 해석하는 최소 길이 (바이트).
const HEADER_MIN_LEN: usize = 48;
/// 시그니처 필드 길이 (바이트).
const SIGNATURE_LEN: usize = 32;
/// HWP5 시그니처 — 32바이트 필드를 공백·NUL 로 다듬은 값.
const HWP5_SIGNATURE: &str = "HWP Document File";
/// HWP 3.0 이하 시그니처의 선행부 — `"HWP Document File V3.00 ..."` 형태다.
const HWP3_SIGNATURE_PREFIX: &str = "HWP Document File V3";
/// 버전 필드 오프셋 (바이트 순서가 뒤집혀 저장된다).
const VERSION_OFFSET: usize = 32;
/// 1차 속성 플래그 바이트 오프셋.
const FLAGS_OFFSET: usize = 36;
/// 2차 속성 플래그 바이트 오프셋.
const EXT_FLAGS_OFFSET: usize = 37;

/// HWPX 패키지의 mimetype 값.
pub const HWPX_MIMETYPE: &str = "application/hwp+zip";
/// HWPX manifest 에서 암호화된 구성 요소를 나타내는 요소 이름.
const MANIFEST_ENCRYPTION_TAG: &str = "encryption-data";

/// 배포용 문서 안내 — H2Orestart 0.7.11+ 는 지원하므로 거부하지 않는다.
pub const NOTE_DISTRIBUTABLE: &str =
    "배포용(읽기 전용) 한글 문서입니다. 일부 서식이 원본과 다르게 보일 수 있습니다.";
/// HWPX manifest 에 암호화 항목이 있을 때의 안내.
pub const NOTE_HWPX_ENCRYPTION_DATA: &str =
    "일부 구성 요소가 암호화된 문서입니다. 해당 부분이 결과물에서 누락될 수 있습니다.";

/// HWP5 FileHeader 의 속성 플래그 중 변환 가부에 영향을 주는 것들.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Hwp5Flags {
    /// 본문이 압축되어 있는가 (변환 가부와 무관, 진단용).
    pub compressed: bool,
    /// 암호로 잠긴 문서인가.
    pub password_encrypted: bool,
    /// 배포용(읽기 전용) 문서인가.
    pub distributable: bool,
    /// 스크립트를 저장했는가 (변환 가부와 무관, 진단용).
    pub save_script: bool,
    /// DRM 보안 문서인가.
    pub drm_protected: bool,
    /// 공인인증서 기반으로 암호화된 문서인가.
    pub pki_encrypted: bool,
    /// 공인인증서 기반 DRM 문서인가.
    pub pki_certificate_drm: bool,
}

/// HWP5 FileHeader 스트림에서 뽑아낸 판정 근거.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hwp5Header {
    /// 문서 버전 `(major, minor, micro, patch)`.
    pub version: (u8, u8, u8, u8),
    /// 속성 플래그.
    pub flags: Hwp5Flags,
}

/// FileHeader 를 해석할 수 없는 경우.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PreflightError {
    /// 시그니처가 HWP5 가 아니다.
    #[error("HWP5 시그니처가 아닙니다")]
    NotHwp5Signature,
    /// FileHeader 스트림이 최소 길이에 못 미친다.
    #[error("FileHeader 스트림이 너무 짧습니다 ({actual}바이트, 최소 {expected}바이트)")]
    HeaderTooShort {
        /// 실제로 주어진 길이.
        actual: usize,
        /// 필요한 최소 길이.
        expected: usize,
    },
    /// HWP 3.0 이하 형식이다 (CFB 가 아니며 H2Orestart 가 다루지 못한다).
    #[error("HWP 3.0 이하 형식입니다")]
    LegacyHwpV3,
}

impl PreflightError {
    /// 해석 실패를 사용자에게 보여줄 거부 사유로 옮긴다.
    pub fn reject_reason(&self) -> RejectReason {
        match self {
            Self::LegacyHwpV3 => RejectReason::UnsupportedHwpV3,
            Self::NotHwp5Signature | Self::HeaderTooShort { .. } => RejectReason::NotHwpDocument,
        }
    }
}

/// 변환을 거부하는 사유.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// 암호가 걸려 있다 (일반 암호 또는 공인인증서 암호화).
    PasswordProtected,
    /// DRM(보안)이 적용돼 있다.
    DrmProtected,
    /// HWP 3.0 이하 형식이다.
    UnsupportedHwpV3,
    /// 한글 문서 형식이 아니다.
    NotHwpDocument,
}

impl RejectReason {
    /// 모든 거부 사유 — 메시지 누락을 테스트에서 전수 확인하기 위해 둔다.
    pub const ALL: [RejectReason; 4] = [
        RejectReason::PasswordProtected,
        RejectReason::DrmProtected,
        RejectReason::UnsupportedHwpV3,
        RejectReason::NotHwpDocument,
    ];
}

/// 프리플라이트 판정 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Preflight {
    /// 그대로 변환을 진행한다.
    Proceed,
    /// 변환은 진행하되 사용자에게 안내를 함께 보여준다.
    ProceedWithNote(&'static str),
    /// 변환을 시작하지 않고 거부한다.
    Reject(RejectReason),
}

/// HWP5 의 `FileHeader` 스트림 바이트를 해석한다.
///
/// CFB 를 여는 일은 호출자 책임이며, 이 함수는 바이트 슬라이스만 보는 순수 함수다.
pub fn parse_hwp5_header(header: &[u8]) -> Result<Hwp5Header, PreflightError> {
    if header.len() < HEADER_MIN_LEN {
        return Err(PreflightError::HeaderTooShort {
            actual: header.len(),
            expected: HEADER_MIN_LEN,
        });
    }
    verify_signature(&header[..SIGNATURE_LEN])?;

    let raw_version = &header[VERSION_OFFSET..VERSION_OFFSET + 4];
    Ok(Hwp5Header {
        // 버전은 바이트 순서가 뒤집혀 저장된다: buf[35].buf[34].buf[33].buf[32]
        version: (
            raw_version[3],
            raw_version[2],
            raw_version[1],
            raw_version[0],
        ),
        flags: parse_flags(header[FLAGS_OFFSET], header[EXT_FLAGS_OFFSET]),
    })
}

/// 32바이트 시그니처 필드를 확인한다. HWP 3.0 이하는 전용 오류로 구분한다.
fn verify_signature(raw: &[u8]) -> Result<(), PreflightError> {
    let text = String::from_utf8_lossy(raw);
    let trimmed = text.trim_matches(|c: char| c.is_whitespace() || c == '\0');

    if trimmed.starts_with(HWP3_SIGNATURE_PREFIX) {
        return Err(PreflightError::LegacyHwpV3);
    }
    if trimmed != HWP5_SIGNATURE {
        return Err(PreflightError::NotHwp5Signature);
    }
    Ok(())
}

/// 속성 플래그 두 바이트를 구조체로 편다.
fn parse_flags(primary: u8, extended: u8) -> Hwp5Flags {
    Hwp5Flags {
        compressed: has_bit(primary, 0),
        password_encrypted: has_bit(primary, 1),
        distributable: has_bit(primary, 2),
        save_script: has_bit(primary, 3),
        drm_protected: has_bit(primary, 4),
        pki_encrypted: has_bit(extended, 0),
        pki_certificate_drm: has_bit(extended, 2),
    }
}

/// `byte` 의 `index` 번째 비트가 서 있는가.
fn has_bit(byte: u8, index: u8) -> bool {
    byte & (1u8 << index) != 0
}

/// 해석된 HWP5 헤더로 변환 가부를 판정한다.
pub fn classify_hwp5(header: &Hwp5Header) -> Preflight {
    let flags = header.flags;

    // 암호와 DRM 이 함께 서 있으면 사용자가 바로 조치할 수 있는 암호를 먼저 알린다.
    if flags.password_encrypted || flags.pki_encrypted {
        return Preflight::Reject(RejectReason::PasswordProtected);
    }
    if flags.drm_protected || flags.pki_certificate_drm {
        return Preflight::Reject(RejectReason::DrmProtected);
    }
    // 배포용은 H2Orestart 0.7.11+ 가 다룰 수 있으므로 거부하지 않는다.
    if flags.distributable {
        return Preflight::ProceedWithNote(NOTE_DISTRIBUTABLE);
    }
    Preflight::Proceed
}

/// HWPX 패키지의 mimetype 과 (있다면) `META-INF/manifest.xml` 내용으로 변환 가부를 판정한다.
///
/// manifest 가 없어도 진행한다 — H2Orestart 0.7.13 에서 고쳐진 회귀를 우리가 되살리지 않는다.
pub fn classify_hwpx(mimetype: &str, manifest_xml: Option<&str>) -> Preflight {
    let normalized = mimetype.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    if normalized != HWPX_MIMETYPE {
        return Preflight::Reject(RejectReason::NotHwpDocument);
    }

    match manifest_xml {
        Some(xml) if xml.contains(MANIFEST_ENCRYPTION_TAG) => {
            Preflight::ProceedWithNote(NOTE_HWPX_ENCRYPTION_DATA)
        }
        _ => Preflight::Proceed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read, Write};

    /// 플래그 바이트를 지정해 48바이트짜리 정상 FileHeader 를 만든다.
    fn 헤더_바이트(flags: u8, ext_flags: u8) -> [u8; HEADER_MIN_LEN] {
        let mut buf = [0u8; HEADER_MIN_LEN];
        buf[..HWP5_SIGNATURE.len()].copy_from_slice(HWP5_SIGNATURE.as_bytes());
        // 버전 5.0.5.0 — 저장 순서는 뒤집혀 있다.
        buf[VERSION_OFFSET..VERSION_OFFSET + 4].copy_from_slice(&[0, 5, 0, 5]);
        buf[FLAGS_OFFSET] = flags;
        buf[EXT_FLAGS_OFFSET] = ext_flags;
        buf
    }

    /// CFB 를 메모리에 만들어 FileHeader 스트림을 쓴 뒤 다시 읽어 온다.
    fn cfb_왕복(header: &[u8]) -> Vec<u8> {
        let mut 문서 = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
        {
            let mut stream = 문서.create_stream("/FileHeader").unwrap();
            stream.write_all(header).unwrap();
            stream.flush().unwrap();
        }
        let 바이트 = 문서.into_inner().into_inner();

        let mut 다시열기 = cfb::CompoundFile::open(Cursor::new(바이트)).unwrap();
        let mut 결과 = Vec::new();
        다시열기
            .open_stream("/FileHeader")
            .unwrap()
            .read_to_end(&mut 결과)
            .unwrap();
        결과
    }

    // ── happy path ──

    #[test]
    fn 플래그가_없는_문서는_그대로_진행한다() {
        // Arrange
        let bytes = 헤더_바이트(0b0000_0001, 0); // compressed 만 켜짐
        let header = parse_hwp5_header(&bytes).unwrap();

        // Act
        let 판정 = classify_hwp5(&header);

        // Assert
        assert!(header.flags.compressed);
        assert_eq!(판정, Preflight::Proceed);
    }

    #[test]
    fn 버전_바이트는_역순으로_해석된다() {
        // Arrange
        let bytes = 헤더_바이트(0, 0); // 32..36 = [0, 5, 0, 5]

        // Act
        let header = parse_hwp5_header(&bytes).unwrap();

        // Assert
        assert_eq!(header.version, (5, 0, 5, 0));
    }

    #[test]
    fn cfb로_합성한_스트림도_동일하게_해석된다() {
        // Arrange
        let bytes = 헤더_바이트(0b0000_0001, 0);
        let 왕복 = cfb_왕복(&bytes);

        // Act
        let header = parse_hwp5_header(&왕복).unwrap();

        // Assert
        assert_eq!(header.version, (5, 0, 5, 0));
        assert_eq!(classify_hwp5(&header), Preflight::Proceed);
    }

    // ── edge cases ──

    #[test]
    fn 암호가_걸린_문서는_변환_전에_거부된다() {
        // Arrange
        let bytes = 헤더_바이트(0b0000_0010, 0); // bit1 passwordEncrypted
        let header = parse_hwp5_header(&bytes).unwrap();

        // Act
        let 판정 = classify_hwp5(&header);

        // Assert
        assert!(header.flags.password_encrypted);
        assert_eq!(판정, Preflight::Reject(RejectReason::PasswordProtected));
    }

    #[test]
    fn drm_문서는_거부된다() {
        // Arrange
        let bytes = 헤더_바이트(0b0001_0000, 0); // bit4 drmProtected
        let header = parse_hwp5_header(&bytes).unwrap();

        // Act
        let 판정 = classify_hwp5(&header);

        // Assert
        assert!(header.flags.drm_protected);
        assert_eq!(판정, Preflight::Reject(RejectReason::DrmProtected));
    }

    #[test]
    fn 공인인증서_암호화_문서는_암호_사유로_거부된다() {
        // Arrange
        let bytes = 헤더_바이트(0, 0b0000_0001); // 37번 바이트 bit0 pkiEncrypted
        let header = parse_hwp5_header(&bytes).unwrap();

        // Act
        let 판정 = classify_hwp5(&header);

        // Assert
        assert!(header.flags.pki_encrypted);
        assert_eq!(판정, Preflight::Reject(RejectReason::PasswordProtected));
    }

    #[test]
    fn 공인인증서_drm_문서는_drm_사유로_거부된다() {
        // Arrange
        let bytes = 헤더_바이트(0, 0b0000_0100); // 37번 바이트 bit2 pkiCertificateDRM
        let header = parse_hwp5_header(&bytes).unwrap();

        // Act
        let 판정 = classify_hwp5(&header);

        // Assert
        assert!(header.flags.pki_certificate_drm);
        assert_eq!(판정, Preflight::Reject(RejectReason::DrmProtected));
    }

    #[test]
    fn 배포용_문서는_거부하지_않고_안내와_함께_진행한다() {
        // Arrange
        let bytes = 헤더_바이트(0b0000_0100, 0); // bit2 distributable
        let header = parse_hwp5_header(&bytes).unwrap();

        // Act
        let 판정 = classify_hwp5(&header);

        // Assert
        assert!(header.flags.distributable);
        assert_eq!(판정, Preflight::ProceedWithNote(NOTE_DISTRIBUTABLE));
    }

    #[test]
    fn 암호와_배포용이_함께_서면_암호를_보고한다() {
        // Arrange
        let bytes = 헤더_바이트(0b0000_0110, 0); // password + distributable
        let header = parse_hwp5_header(&bytes).unwrap();

        // Act
        let 판정 = classify_hwp5(&header);

        // Assert
        assert_eq!(판정, Preflight::Reject(RejectReason::PasswordProtected));
    }

    #[test]
    fn 암호와_drm이_함께_서면_암호를_우선_보고한다() {
        // Arrange
        let bytes = 헤더_바이트(0b0001_0010, 0b0000_0100); // password + drm + pkiCertificateDRM
        let header = parse_hwp5_header(&bytes).unwrap();

        // Act
        let 판정 = classify_hwp5(&header);

        // Assert — 사용자가 즉시 조치할 수 있는 암호를 먼저 알린다
        assert_eq!(판정, Preflight::Reject(RejectReason::PasswordProtected));
    }

    #[test]
    fn 시그니처가_다르면_해석을_거부한다() {
        // Arrange
        let mut bytes = 헤더_바이트(0, 0);
        bytes[..4].copy_from_slice(b"%PDF");

        // Act
        let 결과 = parse_hwp5_header(&bytes);

        // Assert
        assert_eq!(결과, Err(PreflightError::NotHwp5Signature));
        assert_eq!(
            결과.unwrap_err().reject_reason(),
            RejectReason::NotHwpDocument
        );
    }

    #[test]
    fn 헤더가_48바이트보다_짧으면_거부한다() {
        // Arrange
        let bytes = 헤더_바이트(0, 0);
        let 잘린_헤더 = &bytes[..HEADER_MIN_LEN - 1];

        // Act
        let 결과 = parse_hwp5_header(잘린_헤더);

        // Assert
        assert_eq!(
            결과,
            Err(PreflightError::HeaderTooShort {
                actual: 47,
                expected: HEADER_MIN_LEN
            })
        );
    }

    #[test]
    fn hwp_v3_시그니처는_전용_사유로_거부된다() {
        // Arrange — HWP 3.0 은 CFB 가 아니며 시그니처 뒤에 제어 문자가 붙는다
        let mut bytes = [0u8; HEADER_MIN_LEN];
        let sig = b"HWP Document File V3.00 \x1a\x01\x02\x03\x04\x05";
        bytes[..sig.len()].copy_from_slice(sig);

        // Act
        let 결과 = parse_hwp5_header(&bytes);

        // Assert
        assert_eq!(결과, Err(PreflightError::LegacyHwpV3));
        assert_eq!(
            결과.unwrap_err().reject_reason(),
            RejectReason::UnsupportedHwpV3
        );
    }

    #[test]
    fn hwpx는_manifest가_없어도_진행한다() {
        // Arrange & Act — 0.7.13 에서 고쳐진 회귀: manifest 부재를 실패로 보지 않는다
        let 판정 = classify_hwpx(HWPX_MIMETYPE, None);

        // Assert
        assert_eq!(판정, Preflight::Proceed);
    }

    #[test]
    fn hwpx_manifest에_암호화_항목이_있으면_안내와_함께_진행한다() {
        // Arrange
        let manifest = r#"<manifest:manifest xmlns:manifest="...">
  <manifest:file-entry manifest:full-path="Contents/section0.xml">
    <manifest:encryption-data manifest:checksum="abc"/>
  </manifest:file-entry>
</manifest:manifest>"#;

        // Act
        let 판정 = classify_hwpx(HWPX_MIMETYPE, Some(manifest));

        // Assert
        assert_eq!(판정, Preflight::ProceedWithNote(NOTE_HWPX_ENCRYPTION_DATA));
    }

    #[test]
    fn hwpx_mimetype이_다르면_거부한다() {
        // Arrange & Act
        let 판정 = classify_hwpx("application/vnd.oasis.opendocument.text", None);

        // Assert
        assert_eq!(판정, Preflight::Reject(RejectReason::NotHwpDocument));
    }

    #[test]
    fn hwpx_mimetype의_앞뒤_공백과_널문자는_무시한다() {
        // Arrange — ZIP 안의 mimetype 엔트리는 개행이나 NUL 이 섞여 들어올 수 있다
        let 지저분한_mimetype = "  application/hwp+zip\n\0";

        // Act
        let 판정 = classify_hwpx(지저분한_mimetype, None);

        // Assert
        assert_eq!(판정, Preflight::Proceed);
    }
}
