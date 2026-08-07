---
oculpm_discussion: v1
id: file-converter-stack
title: "설치형 파일 변환기 — 스택·변환 엔진 결정"
status: resolved
created: 2026-08-07
updated: 2026-08-07
owner: claude-code
---

## 문제 정의

HWP/HWPX → PDF를 포함한 범용 파일 변환 데스크톱 앱을 만든다. 웹 업로드형이 아닌 **설치형**인 이유: (1) 대용량 파일 변환 시 업로드/다운로드 병목 제거, (2) 문서가 외부 서버로 나가지 않는 보안 — 모든 변환은 로컬에서 수행한다.

- **타깃 플랫폼**: Windows + macOS (크로스플랫폼).
- **변환 범위**: ① HWP/HWPX→PDF ② 이미지 포맷 상호 변환(PNG/JPG/WebP/HEIC 등) ③ Office 문서(DOCX/XLSX/PPTX)→PDF ④ PDF 유틸리티(PDF→이미지, 병합/분할) ⑤ 영상/음성(MP4, MP3 등) — 단계적 릴리스.
- **첫 데모 한 장면**: .hwp/.hwpx 파일을 창에 드래그&드롭하면 로컬에서 PDF로 변환되어 저장된다.
- **배포**: GitHub bunhine0452 계정, 퍼블릭 오픈소스.
- **디자인**: 고급스럽고 편안한 UX — 정교하게 설계된 UI가 1급 요구사항.

결정할 것: (a) 데스크톱 셸/프레임워크, (b) HWP/HWPX 변환 엔진(로컬, 오픈소스 라이선스 호환), (c) 이미지/Office/PDF/미디어 각 변환 백엔드와 번들 전략.

## 후보 해결 방안

**공통 전제 (양안 동일)** — HWP/HWPX→PDF는 오픈소스 로컬 변환 경로가 사실상 하나뿐: **LibreOffice headless + H2Orestart 확장**.
- H2Orestart v0.7.13 (2026-06-27, GPLv3) — HWP v5/HWPX 임포트 필터, `soffice --headless --convert-to pdf:writer_pdf_Export` 동작 확인. 출처: [github.com/ebandal/H2Orestart](https://github.com/ebandal/H2Orestart), Dangerzone(freedomofpress)이 실사용.
- LibreOffice 26.2.5 (2026-07-22, MPL-2.0 재배포 가능). Office 문서(DOCX/XLSX/PPTX)→PDF도 같은 엔진으로 해결.
- 외부 프로세스 호출이므로 앱 본체 라이선스(MIT 등)와 GPL 충돌 없음. 한계: 배포용/DRM HWP 불가, 복잡 레이아웃 재현율 100% 아님.
- 미디어(영상/음성)는 양안 모두 FFmpeg 사이드카(LGPL 빌드) 방식.

### 방안 A — Tauri 2 + React 19 (Rust 코어 + 사이드카) {#opt-tauri}
- 구성: Tauri 2.11.5 (2026-07-01) + React 19 + Vite + Tailwind CSS v4(@tailwindcss/vite) + Vitest / Rust 코어 + `bundle.externalBin` 사이드카.
- PDF 유틸: pdfium-render 0.9.2 (2026-06-13, Chromium PDFium 래퍼) — PDF→이미지·병합/분할.
- 이미지: Rust image 크레이트 + libheif(HEIC). HEIC(HEVC 특허)는 별도 네이티브 의존 — 리스크.
- 장점: 설치본 ~10MB대(웹뷰 내장 안 함), 대용량 파일에 유리한 메모리 프로필, Rust 안정성. 오픈소스 배포 시 다운로드 부담 최소.
- 단점: Rust 학습 곡선, macOS 사이드카 서명 요구, HEIC 파이프라인 직접 구성.
- 출처: [v2.tauri.app/develop/sidecar](https://v2.tauri.app/develop/sidecar/), [crates.io/pdfium-render](https://crates.io/crates/pdfium-render)

### 방안 B — Electron + React 19 (Node 생태계 올인) {#opt-electron}
- 구성: Electron 42.8.1 (2026-08) + 동일 프론트 스택. 변환은 전부 Node에서.
- 이미지: sharp 0.35.3 (libvips LGPL-2.1+) — AVIF 기본 지원, 단 HEIC(HEVC)는 커스텀 libvips 필요.
- PDF: pdf-lib(MIT, 병합/분할) + PDFium 바인딩(렌더). 미디어: ffmpeg-static 패턴 성숙.
- 장점: TypeScript 단일 언어, sharp/ffmpeg-static 등 검증된 배포 패턴, 개발 속도 최고.
- 단점: 설치본 100MB+, 메모리 무거움(대용량 변환 시 불리) — "설치형을 택한 이유"와 부분 상충.
- 출처: [electronjs.org/releases](https://releases.electronjs.org/), [sharp.pixelplumbing.com](https://sharp.pixelplumbing.com/install/)

### LibreOffice 획득 전략 (하위 결정) {#opt-lo-strategy}
1. **감지 + 온디맨드 다운로드**: 설치된 LibreOffice 감지, 없으면 최초 HWP 변환 시 앱이 내려받아 관리 (첫 설치 가볍고 UX 매끄러움 — 추천).
2. **완전 번들**: 인스톨러에 LibreOffice 포함 — 오프라인 완결이지만 인스톨러 수백 MB.
3. **감지만**: 없으면 설치 안내 — 구현 최소지만 UX 단절.

## 토의 / 메모
<!-- oculpm:discussion-log begin v1 -->
| 시각 | 작성자 | 내용 |
|---|---|---|
| 2026-08-07T15:00:00+09:00 | claude-code | STAGE 0 확정: Win+mac 크로스플랫폼, 첫 데모는 HWP→PDF 드래그&드롭, 범위는 문서+이미지+Office+PDF유틸+미디어 전부(단계적) |
| 2026-08-07T15:20:00+09:00 | claude-code | 리서치: HWP 로컬 변환은 LibreOffice 26.2.5 + H2Orestart 0.7.13 경로가 유일 현실안. 셸 후보 Tauri 2.11.5 vs Electron 42.8.1 정리 |
| 2026-08-07T15:35:00+09:00 | claude-code | 사용자 확정: 방안 A(Tauri), LO는 감지+온디맨드, 라이선스 MIT, UI는 shadcn/ui 기반 커스텀 디자인 시스템 |
<!-- oculpm:discussion-log end -->

## 결론

**방안 A 채택 — Tauri 2 + React 19 (Rust 코어 + 사이드카)** {#opt-tauri 참조}

- 셸: Tauri 2.11.x / 프론트: React 19 + Vite + Tailwind CSS v4 + Vitest / 코어: Rust (cargo test).
- HWP/HWPX·Office→PDF: LibreOffice 26.2.x headless + H2Orestart 0.7.13 — **감지 + 온디맨드 다운로드** 전략(설치본 감지 우선, 없으면 앱 관리 디렉토리에 다운로드).
- 이미지: Rust image 생태계(+libheif로 HEIC), PDF 유틸: pdfium-render 0.9.x, 미디어: FFmpeg 사이드카(LGPL 빌드).
- UI: shadcn/ui 기반 + 전용 디자인 토큰(고급스럽고 편안한 UX가 1급 요구사항).
- 라이선스: 앱 본체 MIT (GPL/LGPL 도구는 외부 프로세스·사이드카 경계 유지, 라이선스 고지 포함).
- 배포: GitHub bunhine0452 퍼블릭 리포, 단계적 릴리스(첫 데모 = HWP→PDF 드래그&드롭).

근거: 설치형 선택 이유(대용량·보안)와 정합적인 경량·저메모리 셸, HWP 변환의 유일 현실 경로 재사용(Office 변환까지 커버), 오픈소스 배포 시 다운로드 부담 최소.

## 다음 단계

- [x] 스택 리서치 결과로 후보안 작성 {#next-research}
- [ ] 3-depth 구현 계획 생성 (plan_create) {#next-plan}
- [ ] EVALS.md 완료 기준 작성 {#next-evals}
- [ ] .claude/rules 초기 규칙 작성 {#next-rules}
