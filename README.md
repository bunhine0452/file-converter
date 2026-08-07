# File Converter

**파일이 기기를 떠나지 않는 설치형 파일 변환기** — HWP/HWPX→PDF를 시작으로 사무 문서·이미지·PDF·미디어까지, 모든 변환을 로컬에서 수행합니다.

> 🚧 개발 초기 단계입니다. 첫 목표: `.hwp` 파일을 드래그&드롭하면 PDF로 변환되는 데모.

## 왜 설치형인가

- **보안** — 문서가 외부 서버로 업로드되지 않습니다. 변환은 전부 내 컴퓨터에서 일어납니다.
- **대용량** — 업로드/다운로드 없이 수백 MB 파일도 바로 변환합니다.

## 지원 예정 변환

| 카테고리 | 포맷 |
|---|---|
| 한글 문서 | HWP, HWPX → PDF, DOCX, ODT |
| 사무 문서 | DOC(X), XLS(X), PPT(X), ODT/ODS/ODP, RTF, TXT, CSV ↔ 상호 변환·PDF |
| 이미지 | PNG, JPG, WebP, AVIF, HEIC, BMP, TIFF |
| PDF | PDF→이미지, 병합, 분할 |
| 미디어 | MP4, WebM, MOV, MP3, AAC, WAV, FLAC |

## 스택

Tauri 2 + React 19 + Tailwind CSS v4 · Rust 변환 코어 · LibreOffice(H2Orestart) / PDFium / FFmpeg — 전부 로컬 프로세스.

## 플랫폼

Windows · macOS

## 라이선스

앱 본체는 [MIT](LICENSE). 번들·다운로드되는 서드파티 도구의 고지는 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) 참고.
