//! 파일 타입 감지 — 매직 바이트를 1순위, 확장자를 보조 근거로 사용한다.

use std::path::Path;

/// 헤더 판별에 필요한 최소 바이트 수. HWPX 의 `mimetype` 엔트리까지 읽으려면 64B 면 충분하다.
pub const HEADER_PROBE_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Hwp,
    Hwpx,
    Pdf,
    Png,
    Jpeg,
    Docx,
    Xlsx,
    Pptx,
    Unknown,
}

/// 어떤 근거로 타입을 정했는지. 사용자에게 "확장자만 보고 추정했다"를 알릴 수 있어야 한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionSource {
    /// 매직 바이트만으로 확정 (확장자 없음/무의미)
    Magic,
    /// 매직 바이트 + 확장자가 일치
    MagicAndExtension,
    /// 컨테이너(ZIP/OLE)까지만 매직으로 확인하고, 세부 포맷은 확장자로 좁힘
    ContainerAndExtension,
    /// 매직 바이트로 판별 불가 — 확장자에만 의존
    Extension,
    /// 아무 근거도 없음
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Detection {
    pub kind: FileKind,
    pub source: DetectionSource,
    /// 매직 바이트와 확장자가 서로 다른 포맷을 가리킬 때 true (매직 바이트를 신뢰한다)
    pub extension_mismatch: bool,
}

const PDF_MAGIC: &[u8] = b"%PDF-";
const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];
const ZIP_MAGIC: &[u8] = &[0x50, 0x4B, 0x03, 0x04];
const OLE_MAGIC: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// ZIP 첫 엔트리 이름이 놓이는 오프셋 (local file header 는 고정 30바이트).
const ZIP_FIRST_ENTRY_NAME_OFFSET: usize = 30;
const MIMETYPE_ENTRY_NAME: &[u8] = b"mimetype";
const HWPX_MIMETYPE: &[u8] = b"application/hwp+zip";

/// 매직 바이트가 컨테이너까지만 알려주는 경우의 컨테이너 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container {
    /// ZIP 기반 (HWPX·OOXML)
    Zip,
    /// OLE Compound File 기반 (HWP 5.0·레거시 Office)
    Ole,
}

/// 파일명과 헤더 바이트로 타입을 판별한다 (순수 함수 — IO 없음).
pub fn detect_from_parts(file_name: &str, header: &[u8]) -> Detection {
    let extension_kind = kind_from_extension(file_name);
    let has_extension = Path::new(file_name).extension().is_some();

    if let Some(kind) = kind_from_magic(header) {
        return from_magic(kind, extension_kind, has_extension);
    }

    if let Some(container) = container_from_magic(header) {
        return from_container(container, extension_kind);
    }

    if extension_kind == FileKind::Unknown {
        return Detection {
            kind: FileKind::Unknown,
            source: DetectionSource::Unknown,
            extension_mismatch: false,
        };
    }

    Detection {
        kind: extension_kind,
        source: DetectionSource::Extension,
        extension_mismatch: false,
    }
}

/// 실제 파일의 앞부분(`HEADER_PROBE_LEN` 바이트)을 읽어 타입을 판별한다.
pub fn detect_file(path: &Path) -> std::io::Result<Detection> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut header = vec![0u8; HEADER_PROBE_LEN];
    let read = file.read(&mut header)?;
    header.truncate(read);

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    Ok(detect_from_parts(file_name, &header))
}

/// 매직 바이트로 포맷이 확정된 경우의 판정 — 확장자는 일치 여부만 기록한다.
fn from_magic(kind: FileKind, extension_kind: FileKind, has_extension: bool) -> Detection {
    if extension_kind == kind {
        return Detection {
            kind,
            source: DetectionSource::MagicAndExtension,
            extension_mismatch: false,
        };
    }

    Detection {
        kind,
        source: DetectionSource::Magic,
        // 확장자가 아예 없으면 "거짓말"이 아니다 — 불일치로 보지 않는다.
        extension_mismatch: has_extension,
    }
}

/// 컨테이너까지만 확인된 경우의 판정 — 세부 포맷은 확장자로 좁힌다.
fn from_container(container: Container, extension_kind: FileKind) -> Detection {
    if container_holds(container, extension_kind) {
        return Detection {
            kind: extension_kind,
            source: DetectionSource::ContainerAndExtension,
            extension_mismatch: false,
        };
    }

    Detection {
        kind: FileKind::Unknown,
        source: DetectionSource::Unknown,
        extension_mismatch: false,
    }
}

fn kind_from_magic(header: &[u8]) -> Option<FileKind> {
    if header.starts_with(PDF_MAGIC) {
        return Some(FileKind::Pdf);
    }
    if header.starts_with(PNG_MAGIC) {
        return Some(FileKind::Png);
    }
    if header.starts_with(JPEG_MAGIC) {
        return Some(FileKind::Jpeg);
    }
    if zip_mimetype(header) == Some(HWPX_MIMETYPE) {
        return Some(FileKind::Hwpx);
    }
    None
}

/// ZIP 의 첫 엔트리가 비압축 `mimetype` 이면 그 값을 돌려준다 (HWPX·ODF 규약).
fn zip_mimetype(header: &[u8]) -> Option<&[u8]> {
    if !header.starts_with(ZIP_MAGIC) {
        return None;
    }

    let name_end = ZIP_FIRST_ENTRY_NAME_OFFSET + MIMETYPE_ENTRY_NAME.len();
    let name = header.get(ZIP_FIRST_ENTRY_NAME_OFFSET..name_end)?;
    if name != MIMETYPE_ENTRY_NAME {
        return None;
    }

    let value = header.get(name_end..)?;
    if value.starts_with(HWPX_MIMETYPE) {
        return Some(HWPX_MIMETYPE);
    }
    None
}

fn container_from_magic(header: &[u8]) -> Option<Container> {
    if header.starts_with(ZIP_MAGIC) {
        return Some(Container::Zip);
    }
    if header.starts_with(OLE_MAGIC) {
        return Some(Container::Ole);
    }
    None
}

fn container_holds(container: Container, kind: FileKind) -> bool {
    match container {
        Container::Zip => matches!(
            kind,
            FileKind::Hwpx | FileKind::Docx | FileKind::Xlsx | FileKind::Pptx
        ),
        Container::Ole => matches!(kind, FileKind::Hwp),
    }
}

fn kind_from_extension(file_name: &str) -> FileKind {
    let Some(extension) = Path::new(file_name).extension().and_then(|e| e.to_str()) else {
        return FileKind::Unknown;
    };

    match extension.to_ascii_lowercase().as_str() {
        "hwp" => FileKind::Hwp,
        "hwpx" => FileKind::Hwpx,
        "pdf" => FileKind::Pdf,
        "png" => FileKind::Png,
        "jpg" | "jpeg" => FileKind::Jpeg,
        "docx" => FileKind::Docx,
        "xlsx" => FileKind::Xlsx,
        "pptx" => FileKind::Pptx,
        _ => FileKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ZIP local file header + 비압축 `mimetype` 엔트리를 흉내낸 바이트열.
    /// HWPX/ODF 계열은 첫 엔트리가 반드시 `mimetype` 이라 헤더 64B 안에서 식별된다.
    fn zip_with_mimetype(mime: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]); // PK\x03\x04
        bytes.extend_from_slice(&[0x14, 0x00]); // version needed
        bytes.extend_from_slice(&[0x00, 0x00]); // flags
        bytes.extend_from_slice(&[0x00, 0x00]); // compression = stored
        bytes.extend_from_slice(&[0x00; 4]); // mtime/mdate
        bytes.extend_from_slice(&[0x00; 4]); // crc32
        bytes.extend_from_slice(&(mime.len() as u32).to_le_bytes()); // compressed size
        bytes.extend_from_slice(&(mime.len() as u32).to_le_bytes()); // uncompressed size
        bytes.extend_from_slice(&8u16.to_le_bytes()); // file name length = "mimetype"
        bytes.extend_from_slice(&0u16.to_le_bytes()); // extra length
        bytes.extend_from_slice(b"mimetype"); // offset 30
        bytes.extend_from_slice(mime.as_bytes()); // offset 38
        bytes
    }

    fn plain_zip() -> Vec<u8> {
        let mut bytes = vec![0x50, 0x4B, 0x03, 0x04];
        bytes.extend_from_slice(&[0x00; 60]);
        bytes
    }

    fn ole_compound() -> Vec<u8> {
        let mut bytes = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        bytes.extend_from_slice(&[0x00; 56]);
        bytes
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn pdf_매직과_확장자가_모두_일치하면_pdf_로_확정한다() {
        let detection = detect_from_parts("보고서.pdf", b"%PDF-1.7\n...");

        assert_eq!(detection.kind, FileKind::Pdf);
        assert_eq!(detection.source, DetectionSource::MagicAndExtension);
        assert!(!detection.extension_mismatch);
    }

    #[test]
    fn png_매직을_인식한다() {
        let header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

        let detection = detect_from_parts("shot.png", &header);

        assert_eq!(detection.kind, FileKind::Png);
        assert_eq!(detection.source, DetectionSource::MagicAndExtension);
    }

    #[test]
    fn jpeg_매직을_인식하고_jpeg_확장자_변형을_모두_받아준다() {
        let header = [0xFF, 0xD8, 0xFF, 0xE0];

        for name in ["photo.jpg", "photo.jpeg", "photo.JPG"] {
            let detection = detect_from_parts(name, &header);

            assert_eq!(detection.kind, FileKind::Jpeg, "이름: {name}");
            assert_eq!(
                detection.source,
                DetectionSource::MagicAndExtension,
                "이름: {name}"
            );
        }
    }

    #[test]
    fn hwpx_는_zip_내부_mimetype_으로_확정한다() {
        let bytes = zip_with_mimetype("application/hwp+zip");

        let detection = detect_from_parts("계약서.hwpx", &bytes);

        assert_eq!(detection.kind, FileKind::Hwpx);
        assert_eq!(detection.source, DetectionSource::MagicAndExtension);
        assert!(!detection.extension_mismatch);
    }

    #[test]
    fn hwp_는_ole_컨테이너와_확장자로_판별한다() {
        let detection = detect_from_parts("계약서.hwp", &ole_compound());

        assert_eq!(detection.kind, FileKind::Hwp);
        assert_eq!(detection.source, DetectionSource::ContainerAndExtension);
    }

    #[test]
    fn office_ooxml_은_zip_컨테이너와_확장자로_좁힌다() {
        let zip = plain_zip();

        for (name, expected) in [
            ("문서.docx", FileKind::Docx),
            ("표.xlsx", FileKind::Xlsx),
            ("발표.pptx", FileKind::Pptx),
        ] {
            let detection = detect_from_parts(name, &zip);

            assert_eq!(detection.kind, expected, "이름: {name}");
            assert_eq!(
                detection.source,
                DetectionSource::ContainerAndExtension,
                "이름: {name}"
            );
        }
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 확장자가_거짓말하면_매직_바이트를_믿고_불일치를_알린다() {
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

        let detection = detect_from_parts("실은png.jpg", &png);

        assert_eq!(detection.kind, FileKind::Png);
        assert_eq!(detection.source, DetectionSource::Magic);
        assert!(detection.extension_mismatch);
    }

    #[test]
    fn hwp_mimetype_인데_zip_확장자면_hwpx_로_보되_불일치를_알린다() {
        let bytes = zip_with_mimetype("application/hwp+zip");

        let detection = detect_from_parts("archive.zip", &bytes);

        assert_eq!(detection.kind, FileKind::Hwpx);
        assert_eq!(detection.source, DetectionSource::Magic);
        assert!(detection.extension_mismatch);
    }

    #[test]
    fn 헤더가_너무_짧으면_확장자로_폴백한다() {
        let detection = detect_from_parts("깨진파일.pdf", b"%P");

        assert_eq!(detection.kind, FileKind::Pdf);
        assert_eq!(detection.source, DetectionSource::Extension);
        assert!(!detection.extension_mismatch);
    }

    #[test]
    fn 빈_헤더와_모르는_확장자는_unknown_이다() {
        let detection = detect_from_parts("데이터.xyz", b"");

        assert_eq!(detection.kind, FileKind::Unknown);
        assert_eq!(detection.source, DetectionSource::Unknown);
        assert!(!detection.extension_mismatch);
    }

    #[test]
    fn 확장자가_없어도_매직만으로_판별한다() {
        let detection = detect_from_parts("noext", b"%PDF-1.4");

        assert_eq!(detection.kind, FileKind::Pdf);
        assert_eq!(detection.source, DetectionSource::Magic);
        assert!(!detection.extension_mismatch);
    }

    #[test]
    fn 알_수_없는_zip_은_확장자_근거가_없으면_unknown_이다() {
        let detection = detect_from_parts("archive.zip", &plain_zip());

        assert_eq!(detection.kind, FileKind::Unknown);
        assert_eq!(detection.source, DetectionSource::Unknown);
    }

    // ── IO 경로 ──────────────────────────────────────────────────

    #[test]
    fn detect_file_은_실제_파일_앞부분을_읽어_판별한다() {
        let dir = std::env::temp_dir().join("file-converter-detect-test");
        std::fs::create_dir_all(&dir).expect("임시 디렉토리 생성");
        let path = dir.join("sample.pdf");
        std::fs::write(&path, b"%PDF-1.7\n1 0 obj\n").expect("임시 파일 작성");

        let detection = detect_file(&path).expect("감지 성공");

        assert_eq!(detection.kind, FileKind::Pdf);
        assert_eq!(detection.source, DetectionSource::MagicAndExtension);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn detect_file_은_없는_파일에_대해_io_에러를_반환한다() {
        let result = detect_file(Path::new("/definitely/not/here.pdf"));

        assert!(result.is_err());
    }
}
