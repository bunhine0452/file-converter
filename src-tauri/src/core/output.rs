//! 산출물 경로 규칙.
//!
//! 여러 파일을 한 폴더에 쏟아 넣을 때 같은 이름이 겹치면 **말없이 덮어쓰는 것이 최악**이다.
//! 사용자가 저장 대화상자에서 직접 고른 경로(=덮어쓰기에 동의한 경로)와 달리, 폴더만
//! 고른 일괄 변환에서는 동의를 받은 적이 없으므로 겹치지 않는 이름을 새로 만든다.

use std::path::{Path, PathBuf};

const PDF_EXTENSION: &str = "pdf";
/// 이름이 없는 입력의 최후 이름 (정상 경로에서는 쓰이지 않는다).
const FALLBACK_STEM: &str = "output";
/// 번호를 붙여 볼 최대 횟수. 이보다 겹치면 폴더 상태가 이상한 것이다.
const MAX_ATTEMPTS: u32 = 1000;

/// 원본에 대응하는 PDF 파일명 (`보고서.v2.hwp` → `보고서.v2.pdf`).
pub fn pdf_name_for(source: &Path) -> String {
    let stem = source
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| FALLBACK_STEM.to_string());

    format!("{stem}.{PDF_EXTENSION}")
}

/// `dir` 안에서 겹치지 않는 경로. 이미 있으면 ` (1)`, ` (2)` … 를 붙인다.
pub fn unique_output_path(dir: &Path, file_name: &str, exists: &dyn Fn(&Path) -> bool) -> PathBuf {
    let candidate = dir.join(file_name);
    if !exists(&candidate) {
        return candidate;
    }

    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| FALLBACK_STEM.to_string());
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().into_owned())
        .unwrap_or_else(|| PDF_EXTENSION.to_string());

    for attempt in 1..=MAX_ATTEMPTS {
        let candidate = dir.join(format!("{stem} ({attempt}).{extension}"));
        if !exists(&candidate) {
            return candidate;
        }
    }

    // 여기까지 오면 번호로는 못 피한다 — 겹칠 수 없는 이름으로 도망친다.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();

    dir.join(format!("{stem} ({stamp}).{extension}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn taken(paths: &[&str]) -> impl Fn(&Path) -> bool {
        let set: BTreeSet<PathBuf> = paths.iter().map(PathBuf::from).collect();

        move |path: &Path| set.contains(path)
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 원본_이름의_확장자만_pdf_로_바꾼다() {
        assert_eq!(pdf_name_for(Path::new("/tmp/보고서.hwp")), "보고서.pdf");
        assert_eq!(pdf_name_for(Path::new("/tmp/계약서.hwpx")), "계약서.pdf");
    }

    #[test]
    fn 빈_폴더에는_그대로_저장한다() {
        let path = unique_output_path(Path::new("/out"), "보고서.pdf", &taken(&[]));

        assert_eq!(path, PathBuf::from("/out/보고서.pdf"));
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 이미_있으면_번호를_붙여_덮어쓰지_않는다() {
        // 말없이 덮어쓰면 사용자가 어제 만든 PDF 가 사라진다.
        let path = unique_output_path(
            Path::new("/out"),
            "보고서.pdf",
            &taken(&["/out/보고서.pdf"]),
        );

        assert_eq!(path, PathBuf::from("/out/보고서 (1).pdf"));
    }

    #[test]
    fn 번호도_겹치면_다음_번호로_넘어간다() {
        let path = unique_output_path(
            Path::new("/out"),
            "보고서.pdf",
            &taken(&["/out/보고서.pdf", "/out/보고서 (1).pdf"]),
        );

        assert_eq!(path, PathBuf::from("/out/보고서 (2).pdf"));
    }

    #[test]
    fn 이름_중간의_점은_보존된다() {
        assert_eq!(
            pdf_name_for(Path::new("/tmp/보고서.v2.hwp")),
            "보고서.v2.pdf"
        );

        let path = unique_output_path(
            Path::new("/out"),
            "보고서.v2.pdf",
            &taken(&["/out/보고서.v2.pdf"]),
        );
        assert_eq!(path, PathBuf::from("/out/보고서.v2 (1).pdf"));
    }

    #[test]
    fn 대문자_확장자도_소문자_pdf_가_된다() {
        assert_eq!(pdf_name_for(Path::new("/tmp/REPORT.HWPX")), "REPORT.pdf");
    }

    #[test]
    fn 전부_겹쳐도_무한_루프에_빠지지_않는다() {
        // exists 가 항상 참이어도 반환은 해야 한다.
        let path = unique_output_path(Path::new("/out"), "보고서.pdf", &|_| true);

        assert!(path.starts_with("/out"));
        assert!(path.to_string_lossy().ends_with(".pdf"));
    }
}
