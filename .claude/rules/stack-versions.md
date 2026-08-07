# 확정 스택·버전 (2026-08-07 리서치 기준)

이 프로젝트의 스택은 discussion(`file-converter-stack`)에서 리서치로 확정됐다. 버전 업그레이드나 스택 교체는 discussion 갱신 후에만.

- 셸: **Tauri 2.11.x** (사이드카는 `bundle.externalBin`) / 코어: Rust
- 프론트: **React 19 + Vite + Tailwind CSS v4(@tailwindcss/vite) + TypeScript**, UI는 shadcn/ui + 전용 디자인 토큰
- 테스트: **Vitest**(프론트) + **cargo test**(코어)
- HWP/HWPX·Office→PDF: **LibreOffice 26.2.x headless + H2Orestart 0.7.13** — 감지 우선, 미설치 시 온디맨드 다운로드(앱 데이터 디렉토리)
- PDF 유틸: **pdfium-render 0.9.x** / 미디어: **FFmpeg LGPL 빌드 사이드카**
- 패키지 매니저: pnpm
