# EVALS — 설치형 파일 변환기 완료 기준

> 근거: `.oculpm/discussion/file-converter-stack/discussion.md` 결론.
> 각 항목은 실행/재현 가능해야 하며, 스위트 단위로 채점해 맨 아래 기록 표에 남긴다 (run-evals 스킬이 실행·기록).
> 테스트 샘플 파일은 `evals/fixtures/`에 둔다 (대용량 파일은 생성 스크립트로 재현).

## S1 — 기반 (스캐폴드·CI)

- [ ] `pnpm install && pnpm tauri dev`가 mac에서 앱 창을 띄운다 (콘솔 에러 0)
- [ ] `pnpm test`(Vitest)와 `cargo test`가 로컬에서 통과한다
- [ ] `pnpm lint`와 `cargo clippy -- -D warnings`가 통과한다
- [ ] GitHub Actions CI가 main 브랜치에서 그린이다 (lint+test+빌드 매트릭스 mac/win)

## S2 — HWP→PDF (첫 데모 장면)

- [ ] LibreOffice 미설치 상태에서 앱이 상태를 감지하고 온디맨드 다운로드를 제안한다
- [ ] `evals/fixtures/sample.hwp`를 창에 드래그&드롭하면 PDF가 지정 폴더에 저장된다
- [ ] 생성된 PDF를 열었을 때 원본의 텍스트·표·이미지가 육안 대조로 재현된다 (샘플 3종: 텍스트 위주 / 표 포함 / 이미지 포함)
- [ ] `evals/fixtures/sample.hwpx`도 동일 경로로 변환된다
- [ ] 암호 걸린 HWP 투입 시 앱이 멈추지 않고 한국어 친화 에러 메시지를 표시한다
- [ ] 100MB급 HWP(생성 스크립트 `evals/scripts/make-large-hwp`) 변환 중 UI가 응답하고 진행 표시가 유지된다
- [ ] 변환 전 과정에서 파일 내용이 네트워크로 전송되지 않는다 (프록시 캡처로 검증 — 도구 바이너리 다운로드 트래픽만 허용)

## S3 — 디자인·UX

- [ ] 라이트/다크 테마 전환이 즉시 반영되고 모든 화면에서 WCAG AA 대비를 통과한다 (자동 검사 도구 기준)
- [ ] 키보드만으로 파일 선택→변환→결과 열기가 가능하다
- [ ] 드롭존이 드래그 오버/유효/무효 상태를 시각적으로 구분해 보여준다
- [ ] OS `prefers-reduced-motion` 설정 시 장식성 애니메이션이 비활성화된다
- [ ] 첫 실행부터 변환 완료까지 설명 없이 진행 가능하다 (신규 사용자 1인 이상 관찰 테스트)

## S4 — 이미지 변환

- [ ] PNG↔JPG↔WebP↔AVIF 매트릭스 변환이 모두 성공하고 결과 파일이 뷰어에서 열린다
- [ ] HEIC(아이폰 촬영 샘플)→JPG 변환이 성공한다
- [ ] 품질/리사이즈 옵션이 결과물 크기·해상도에 실제 반영된다
- [ ] 이미지 20장 일괄 투입 시 전체 진행률이 표시되고 모두 변환된다

## S5 — 사무 문서 전방위·PDF 유틸리티

- [ ] 한글 텍스트 포함 DOCX/XLSX/PPTX→PDF가 각각 성공하고 글자 깨짐이 없다
- [ ] HWP→DOCX 변환 결과가 Word/LibreOffice에서 편집 가능하게 열린다
- [ ] 레거시 DOC/XLS/PPT→PDF 변환이 성공한다
- [ ] DOCX↔ODT↔RTF↔TXT, PPTX↔ODP 상호 변환이 샘플로 성공한다
- [ ] XLSX↔CSV 변환에서 UTF-8/CP949 인코딩 선택이 결과에 반영된다 (한글 셀 깨짐 없음)
- [ ] PDF→PNG(DPI 300, 페이지 범위 지정)가 성공한다
- [ ] PDF 2개 병합(순서 지정)과 페이지 범위 분할이 성공한다

## S6 — 미디어 변환

- [ ] MP4→WebM, MOV→MP4 변환이 프리셋으로 성공하고 재생 가능하다
- [ ] MP4→MP3 음성 추출, WAV→FLAC 변환이 성공한다
- [ ] 1GB급 영상 변환 중 진행률이 표시되고 취소가 즉시 동작한다

## S7 — 릴리스 게이트

- [ ] mac(dmg)/win(nsis) 설치본이 클린 VM/머신에서 설치·실행된다
- [ ] THIRD-PARTY-NOTICES에 LibreOffice·H2Orestart·FFmpeg·PDFium 고지가 포함된다
- [ ] README의 설치 안내만 보고 제3자가 설치에 성공한다

---

## 기록

| 날짜 | 스위트 | 통과 | 메모 |
|---|---|---|---|
