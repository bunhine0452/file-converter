//! 실제 파일을 열어 프리플라이트한다.
//!
//! HWP5 는 OLE 복합문서의 `FileHeader` 스트림을, HWPX 는 ZIP 의 `mimetype` 과
//! `META-INF/manifest.xml` 을 읽는다. 판정 규칙 자체는 [`super::preflight`] 의 순수 함수에 있다.

use std::io::Read;
use std::path::Path;

use crate::core::file_type::{detect_file, FileKind};
use crate::core::hwp::preflight::{
    classify_hwp5, classify_hwpx, parse_hwp5_header, Preflight, RejectReason, HEADER_MIN_LEN,
};

/// HWP5 복합문서 안에서 헤더가 들어 있는 스트림 이름.
const FILE_HEADER_STREAM: &str = "/FileHeader";
const HWPX_MIMETYPE_ENTRY: &str = "mimetype";
const HWPX_MANIFEST_ENTRY: &str = "META-INF/manifest.xml";

#[derive(Debug, thiserror::Error)]
pub enum InspectError {
    #[error("파일을 읽지 못했습니다: {0}")]
    Io(String),
    #[error("문서 구조를 해석하지 못했습니다: {0}")]
    Malformed(String),
}

/// 변환을 시작해도 되는지 판정한다. 여기서 막지 못하면 사용자가 빈 PDF 를 받는다.
pub fn preflight_file(path: &Path) -> Result<Preflight, InspectError> {
    let detection = detect_file(path).map_err(|error| InspectError::Io(error.to_string()))?;

    match detection.kind {
        FileKind::Hwp => preflight_hwp5(path),
        FileKind::Hwpx => preflight_hwpx(path),
        _ => Ok(Preflight::Reject(RejectReason::NotHwpDocument)),
    }
}

fn preflight_hwp5(path: &Path) -> Result<Preflight, InspectError> {
    let file = std::fs::File::open(path).map_err(|error| InspectError::Io(error.to_string()))?;
    let mut compound = cfb::CompoundFile::open(file)
        .map_err(|error| InspectError::Malformed(error.to_string()))?;
    let mut stream = compound
        .open_stream(FILE_HEADER_STREAM)
        .map_err(|error| InspectError::Malformed(error.to_string()))?;

    let mut header = vec![0u8; HEADER_MIN_LEN];
    let read = stream
        .read(&mut header)
        .map_err(|error| InspectError::Io(error.to_string()))?;
    header.truncate(read);

    match parse_hwp5_header(&header) {
        Ok(parsed) => Ok(classify_hwp5(&parsed)),
        Err(error) => Err(InspectError::Malformed(error.to_string())),
    }
}

fn preflight_hwpx(path: &Path) -> Result<Preflight, InspectError> {
    let file = std::fs::File::open(path).map_err(|error| InspectError::Io(error.to_string()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| InspectError::Malformed(error.to_string()))?;

    let mimetype = read_entry(&mut archive, HWPX_MIMETYPE_ENTRY).unwrap_or_default();
    // manifest 는 없을 수 있다 — 없다고 실패로 단정하지 않는다.
    let manifest = read_entry(&mut archive, HWPX_MANIFEST_ENTRY);

    Ok(classify_hwpx(&mimetype, manifest.as_deref()))
}

fn read_entry<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    let mut contents = String::new();
    entry.read_to_string(&mut contents).ok()?;

    Some(contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    /// 플래그 바이트만 다른 최소 HWP5 문서를 만든다 — 실제 한글 파일 없이 모든 조합을 시험한다.
    fn write_hwp5(path: &Path, flag_byte_36: u8) {
        let mut header = vec![0u8; HEADER_MIN_LEN];
        header[..17].copy_from_slice(b"HWP Document File");
        // 버전 5.0.5.0 (바이트 순서가 뒤집혀 저장된다)
        header[32..36].copy_from_slice(&[0, 5, 0, 5]);
        header[36] = flag_byte_36;

        let mut compound =
            cfb::CompoundFile::create(Cursor::new(Vec::new())).expect("복합문서 생성");
        let mut stream = compound
            .create_stream(FILE_HEADER_STREAM)
            .expect("스트림 생성");
        stream.write_all(&header).expect("헤더 기록");
        drop(stream);

        let bytes = compound.into_inner().into_inner();
        std::fs::write(path, bytes).expect("파일 기록");
    }

    fn write_hwpx(path: &Path, mimetype: &str, manifest: Option<&str>) {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let stored = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file(HWPX_MIMETYPE_ENTRY, stored)
                .expect("mimetype 시작");
            zip.write_all(mimetype.as_bytes()).expect("mimetype 기록");

            if let Some(manifest) = manifest {
                zip.start_file(HWPX_MANIFEST_ENTRY, stored)
                    .expect("manifest 시작");
                zip.write_all(manifest.as_bytes()).expect("manifest 기록");
            }
            zip.finish().expect("zip 마무리");
        }

        std::fs::write(path, buffer.into_inner()).expect("파일 기록");
    }

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fc-inspect-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("시간")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("임시 디렉토리");
        dir
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 평범한_hwp5_는_변환을_진행한다() {
        let dir = temp_dir();
        let path = dir.join("문서.hwp");
        write_hwp5(&path, 0b0000_0001); // compressed 만 켜짐

        let verdict = preflight_file(&path).expect("판정");

        assert_eq!(verdict, Preflight::Proceed);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 정상_hwpx_는_변환을_진행한다() {
        let dir = temp_dir();
        let path = dir.join("문서.hwpx");
        write_hwpx(&path, "application/hwp+zip", Some("<manifest></manifest>"));

        let verdict = preflight_file(&path).expect("판정");

        assert_eq!(verdict, Preflight::Proceed);
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 암호_걸린_hwp5_는_변환_전에_거부된다() {
        // 이걸 놓치면 사용자는 아무 에러 없이 빈 PDF 를 받는다.
        let dir = temp_dir();
        let path = dir.join("암호.hwp");
        write_hwp5(&path, 0b0000_0010); // bit1 = passwordEncrypted

        let verdict = preflight_file(&path).expect("판정");

        assert_eq!(verdict, Preflight::Reject(RejectReason::PasswordProtected));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn drm_문서도_거부된다() {
        let dir = temp_dir();
        let path = dir.join("drm.hwp");
        write_hwp5(&path, 0b0001_0000); // bit4 = drmProtected

        let verdict = preflight_file(&path).expect("판정");

        assert_eq!(verdict, Preflight::Reject(RejectReason::DrmProtected));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 배포용_문서는_안내만_남기고_통과한다() {
        let dir = temp_dir();
        let path = dir.join("배포용.hwp");
        write_hwp5(&path, 0b0000_0100); // bit2 = distributable

        let verdict = preflight_file(&path).expect("판정");

        assert!(matches!(verdict, Preflight::ProceedWithNote(_)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn manifest_가_없는_hwpx_도_통과한다() {
        let dir = temp_dir();
        let path = dir.join("무매니페스트.hwpx");
        write_hwpx(&path, "application/hwp+zip", None);

        let verdict = preflight_file(&path).expect("판정");

        assert_eq!(verdict, Preflight::Proceed);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 한글_문서가_아니면_거부한다() {
        let dir = temp_dir();
        let path = dir.join("그림.png");
        std::fs::write(&path, [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).expect("기록");

        let verdict = preflight_file(&path).expect("판정");

        assert_eq!(verdict, Preflight::Reject(RejectReason::NotHwpDocument));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 없는_파일은_io_에러다() {
        let result = preflight_file(Path::new("/definitely/not/here.hwp"));

        assert!(matches!(result, Err(InspectError::Io(_))));
    }

    #[test]
    fn ole_컨테이너가_아닌_hwp_확장자는_해석_실패로_보고한다() {
        let dir = temp_dir();
        let path = dir.join("가짜.hwp");
        // OLE 시그니처만 흉내내고 내용은 쓰레기
        let mut bytes = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        bytes.extend_from_slice(&[0x00; 512]);
        std::fs::write(&path, bytes).expect("기록");

        let result = preflight_file(&path);

        assert!(matches!(result, Err(InspectError::Malformed(_))));
        std::fs::remove_dir_all(&dir).ok();
    }
}
