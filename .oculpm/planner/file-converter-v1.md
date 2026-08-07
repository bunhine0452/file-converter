---
oculpm_plan: v1
id: file-converter-v1
title: "설치형 파일 변환기 v0.1 — HWP→PDF부터 미디어까지"
status: active
created: 2026-08-07
updated: 2026-08-07
owner: claude-code
---

Tauri 2 + React 19 크로스플랫폼(Win/mac) 로컬 파일 변환기. 첫 데모는 HWP/HWPX→PDF 드래그&드롭, 이후 이미지·Office·PDF 유틸·미디어로 확장. 근거: .oculpm/discussion/file-converter-stack.

## Phase 1 — 기반: 스캐폴드·저장소·CI {#ph1-foundation}
- [ ] 리서치에서 확정한 버전으로 환경 구성·스캐폴드 (Tauri 2.11.x + React 19 + Vite + Tailwind v4 + TypeScript + Vitest) {#scaffold}
  - [ ] create-tauri-app으로 스캐폴드 후 `pnpm tauri dev`가 빈 창을 띄운다 {#scaffold-app}
  - [ ] Tailwind CSS v4(@tailwindcss/vite)와 shadcn/ui 초기화가 데모 버튼 렌더로 확인된다 {#scaffold-tailwind}
  - [ ] Vitest 샘플 테스트와 `cargo test` 샘플 테스트가 로컬에서 통과한다 {#scaffold-test}
  - [ ] ESLint+Prettier, rustfmt+clippy 설정이 커밋 훅 없이 스크립트로 실행된다 {#scaffold-lint}
- [x] GitHub 퍼블릭 저장소 공개 준비 (bunhine0452/file-converter, MIT) {#repo-setup}
  - [x] git init + 초기 커밋 + gh로 퍼블릭 리포 생성·푸시가 완료된다 {#repo-init}
  - [x] MIT LICENSE, README 뼈대(프로젝트 소개·설치형 이유), .gitignore가 커밋된다 {#repo-license}
  - [x] THIRD-PARTY-NOTICES에 LibreOffice(MPL)·H2Orestart(GPL, 외부 프로세스)·FFmpeg(LGPL)·PDFium 고지 초안이 들어간다 {#repo-notice}
- [ ] CI 빌드 파이프라인이 그린이다 {#ci-pipeline}
  - [ ] GitHub Actions에서 lint+Vitest+cargo test가 PR마다 실행된다 {#ci-test}
  - [ ] mac(aarch64)/win(x64) tauri 빌드 매트릭스가 아티팩트(dmg/nsis)를 생성한다 {#ci-build}
- [ ] 변환 코어 골격 (작업 큐·타입 감지·진행 이벤트) {#core-skeleton}
  - [ ] Rust 변환 작업 큐(등록/취소/상태)가 단위 테스트로 검증된다 {#core-job-queue}
  - [ ] 확장자+매직 바이트 파일 타입 감지가 hwp/hwpx/png/jpg/pdf 샘플로 테스트 통과한다 {#core-detect}
  - [ ] Rust→프론트 진행률 이벤트 브리지가 데모 카운터로 동작한다 {#core-events}

## Phase 2 — 첫 데모: HWP/HWPX → PDF 드래그&드롭 {#ph2-hwp-demo}
- [ ] LibreOffice 런타임 매니저 (감지 + 온디맨드 다운로드) {#lo-runtime}
  - [ ] mac/win 표준 경로·PATH에서 soffice 탐지가 단위 테스트로 검증된다 {#lo-detect}
  - [ ] 미설치 시 LibreOffice 26.2.x를 앱 데이터 디렉토리에 다운로드·해시 검증·설치한다 {#lo-download}
  - [ ] H2Orestart 0.7.13 확장을 unopkg/프로필로 자동 설치하고 설치 여부를 검증한다 {#lo-h2o}
  - [ ] 설정 화면에 LibreOffice 상태(감지됨/다운로드 필요/버전)가 표시된다 {#lo-status-ui}
- [ ] HWP 변환 파이프라인 {#hwp-pipeline}
  - [ ] soffice headless 호출 래퍼(임시 프로필·타임아웃·에러 매핑)가 단위 테스트로 검증된다 {#hwp-wrapper}
  - [ ] .hwp 드래그&드롭 → PDF 저장 happy-path가 실제 샘플 파일로 동작한다 (첫 데모) {#hwp-happy}
  - [ ] .hwpx 동일 경로가 실제 샘플 파일로 동작한다 {#hwpx-happy}
  - [ ] 암호 문서·배포용 문서·손상 파일이 사용자 친화 에러 메시지로 처리된다 {#hwp-errors}
  - [ ] 100MB급 대용량 HWP가 UI 멈춤 없이 변환된다 (진행 표시 유지) {#hwp-large}
- [ ] 데모 UX: 드롭존·진행률·완료 흐름 {#demo-ux}
  - [ ] 드롭존이 드래그 오버/유효·무효 파일 상태를 시각 피드백한다 {#dropzone}
  - [ ] 파일별 진행률·완료·실패 상태가 큐 리스트에 실시간 표시된다 {#progress-ui}
  - [ ] 완료 시 저장 위치 열기/파일 열기 액션이 mac/win에서 동작한다 {#output-open}

## Phase 3 — 디자인 시스템·앱 셸 (고급스럽고 편안한 UX) {#ph3-design}
- [ ] 디자인 토큰·테마 체계 {#design-tokens}
  - [ ] 색·타이포·여백·라운드·그림자·모션 토큰이 Tailwind v4 @theme으로 정의된다 (light/dark) {#tokens-define}
  - [ ] shadcn/ui 컴포넌트가 전용 토큰을 상속해 기본 룩과 구분되는 룩을 갖는다 {#tokens-shadcn}
  - [ ] 한글 최적화 서체 스택(Pretendard 등)과 고정폭 숫자(tabular figures) 설정이 적용된다 {#tokens-typography}
- [ ] 앱 셸 레이아웃 {#app-shell}
  - [ ] 변환 카테고리(문서/이미지/PDF/미디어) 사이드바와 메인 큐 화면 레이아웃이 완성된다 {#shell-nav}
  - [ ] 설정 화면(출력 폴더·이름 규칙·테마·LibreOffice 상태)이 동작한다 {#shell-settings}
  - [ ] 네이티브 타이틀바/윈도 컨트롤이 mac/win 각각 자연스럽게 통합된다 {#shell-window}
- [ ] 디자인 품질·접근성 게이트 {#design-quality}
  - [ ] 키보드만으로 파일 선택→변환→결과 열기가 가능하다 {#a11y-keyboard}
  - [ ] 라이트/다크 모두 WCAG AA 대비를 통과한다 {#a11y-contrast}
  - [ ] 드롭·진행·완료 마이크로 인터랙션이 reduced-motion 설정을 존중한다 {#motion-polish}

## Phase 4 — 이미지 변환 {#ph4-images}
- [ ] 이미지 변환 엔진 (Rust) {#img-engine}
  - [ ] PNG/JPG/WebP/AVIF/BMP/TIFF 상호 변환이 매트릭스 테스트로 검증된다 {#img-basic}
  - [ ] 품질·리사이즈(최대 변)·메타데이터 제거 옵션이 동작한다 {#img-options}
  - [ ] HEIC 디코드(libheif, mac은 시스템 폴백 검토)가 아이폰 샘플 사진으로 검증된다 {#img-heic}
- [ ] 일괄 변환 UX {#img-batch}
  - [ ] 여러 파일 혼합 투입 시 파일별 타깃 포맷 지정·일괄 적용이 동작한다 {#batch-queue}
  - [ ] 이미지 변환이 코어 수 기반 병렬로 처리되고 전체 진행률이 정확하다 {#batch-parallel}

## Phase 5 — 사무 문서 전방위 변환·PDF 유틸리티 {#ph5-office-pdf}
- [ ] Office 문서 → PDF (LibreOffice 재사용) {#office-convert}
  - [ ] DOCX/XLSX/PPTX→PDF가 샘플 문서로 검증된다 (한글 폰트 포함) {#office-docx}
  - [ ] 암호 문서·매크로 문서의 에러 처리가 HWP와 동일한 UX로 동작한다 {#office-errors}
- [ ] 문서 변환 매트릭스 일반화 (사무 포맷 전방위 — LibreOffice 필터 재사용) {#doc-matrix}
  - [ ] 포맷 레지스트리(입력→가능한 출력 매핑)가 단일 소스로 정의되고 UI 선택지가 이를 반영한다 {#matrix-registry}
  - [ ] HWP/HWPX→DOCX/ODT 편집용 변환이 샘플로 검증된다 {#hwp-to-docx}
  - [ ] 레거시 DOC/XLS/PPT→PDF/최신 포맷 변환이 샘플로 검증된다 {#legacy-office}
  - [ ] DOCX↔ODT↔RTF↔TXT 상호 변환이 샘플로 검증된다 {#writer-matrix}
  - [ ] XLSX↔CSV↔ODS 변환이 인코딩 옵션(UTF-8/CP949)과 함께 검증된다 {#sheet-matrix}
  - [ ] PPTX↔ODP 상호 변환이 샘플로 검증된다 {#slide-matrix}
- [ ] PDF 유틸리티 (pdfium-render) {#pdf-utils}
  - [ ] PDF→PNG/JPG(페이지 선택·DPI 지정)가 샘플로 검증된다 {#pdf-to-img}
  - [ ] 여러 PDF 병합(드래그로 순서 지정)이 동작한다 {#pdf-merge}
  - [ ] 페이지 범위 분할·추출이 동작한다 {#pdf-split}

## Phase 6 — 미디어 변환 (FFmpeg) {#ph6-media}
- [ ] FFmpeg 사이드카 통합 {#ffmpeg-sidecar}
  - [ ] LGPL 빌드 FFmpeg가 externalBin 사이드카로 mac/win 번들·실행된다 {#ff-bundle}
  - [ ] FFmpeg 진행률 파싱이 큐 진행 표시에 연결된다 {#ff-progress}
- [ ] 영상·음성 변환 {#media-convert}
  - [ ] MP4/WebM/MOV 상호 변환(해상도·코덱 프리셋)이 샘플로 검증된다 {#video-convert}
  - [ ] MP3/AAC/WAV/FLAC 상호 변환·추출(영상→음성)이 샘플로 검증된다 {#audio-convert}

## Phase 7 — v0.1.0 퍼블릭 릴리스 {#ph7-release}
- [ ] 서명·배포 체계 {#signing}
  - [ ] mac 서명/공증(인증서 확보 시) 또는 unsigned 실행 안내 문서가 준비된다 {#sign-mac}
  - [ ] win SmartScreen 대응 방침(서명 또는 안내)이 결정·문서화된다 {#sign-win}
  - [ ] tauri-updater로 자동 업데이트가 draft 릴리스로 검증된다 {#updater}
- [ ] 오픈소스 문서·릴리스 {#docs-release}
  - [ ] README(스크린샷·기능표·설치 안내 한/영)가 완성된다 {#readme-full}
  - [ ] CONTRIBUTING·이슈 템플릿·라이선스 고지 최종본이 커밋된다 {#contrib-docs}
  - [ ] GitHub Releases에 v0.1.0(dmg/nsis)이 EVALS 통과 기록과 함께 공개된다 {#release-v01}

<!-- oculpm:plan-log begin v1 -->
| 시각 | 항목 | 에이전트 | 변화 | 일지 | 메모 |
|---|---|---|---|---|---|
| 2026-08-07T14:56:46+09:00 | #repo-init | claude-code | ☐→x | .oculpm/journal/20260807/Chores/1456_chore_project-inception-file-converter.md | gh로 퍼블릭 리포 생성·푸시 완료 |
| 2026-08-07T14:56:54+09:00 | #repo-license | claude-code | ☐→x | .oculpm/journal/20260807/Chores/1456_chore_project-inception-file-converter.md | MIT LICENSE·README 뼈대·.gitignore 초기 커밋 포함 |
| 2026-08-07T14:57:03+09:00 | #repo-notice | claude-code | ☐→x | .oculpm/journal/20260807/Chores/1456_chore_project-inception-file-converter.md | THIRD-PARTY-NOTICES 초안(LO·H2Orestart·FFmpeg·PDFium) 커밋 |
<!-- oculpm:plan-log end -->
