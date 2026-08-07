---
schema_version: 1
type: feature
slug: "phase1-scaffold-and-core-skeleton"
status: done
difficulty: high
created_at: "2026-08-07T15:44:59+09:00"
session_id: "mcp-20260807-154459"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "package.json"
    op: create
  - path: "vite.config.ts"
    op: create
  - path: "tsconfig.json"
    op: create
  - path: "tsconfig.node.json"
    op: create
  - path: "eslint.config.js"
    op: create
  - path: ".prettierrc.json"
    op: create
  - path: ".prettierignore"
    op: create
  - path: "components.json"
    op: create
  - path: "index.html"
    op: create
  - path: "src/main.tsx"
    op: create
  - path: "src/index.css"
    op: create
  - path: "src/App.tsx"
    op: create
  - path: "src/App.test.tsx"
    op: create
  - path: "src/test/setup.ts"
    op: create
  - path: "src/lib/utils.ts"
    op: create
  - path: "src/lib/jobs.ts"
    op: create
  - path: "src/lib/jobs.test.ts"
    op: create
  - path: "src/components/ui/button.tsx"
    op: create
  - path: "src/components/JobProgressDemo.tsx"
    op: create
  - path: "src/components/JobProgressDemo.test.tsx"
    op: create
  - path: "src-tauri/Cargo.toml"
    op: create
  - path: "src-tauri/tauri.conf.json"
    op: create
  - path: "src-tauri/rustfmt.toml"
    op: create
  - path: "src-tauri/src/main.rs"
    op: create
  - path: "src-tauri/src/lib.rs"
    op: create
  - path: "src-tauri/src/core/mod.rs"
    op: create
  - path: "src-tauri/src/core/file_type.rs"
    op: create
  - path: "src-tauri/src/core/job.rs"
    op: create
  - path: "src-tauri/src/core/events.rs"
    op: create
  - path: "src-tauri/src/shell/mod.rs"
    op: create
  - path: "src-tauri/src/shell/event_sink.rs"
    op: create
  - path: "src-tauri/src/shell/commands.rs"
    op: create
  - path: ".github/workflows/ci.yml"
    op: create
  - path: ".github/workflows/build.yml"
    op: create
  - path: ".gitignore"
    op: update
related: []
tags:
  - "tauri"
  - "react"
  - "rust"
  - "tdd"
  - "scaffold"
  - "ci"
  - "mcp-tool"
---
[x] Phase 1 — Tauri 스캐폴드와 변환 코어 골격(TDD)

## 추가 기능

Phase 1 의 스캐폴드와 변환 코어 골격을 세웠다. 코어 3종(#core-detect·#core-job-queue·#core-events)은 전부 TDD 로 진행 — 실패를 먼저 눈으로 확인한 뒤 최소 구현했다.

- **스캐폴드**: create-tauri-app(react-ts) 산출물을 저장소로 옮기고 이름·식별자 정리. Tauri 2.11.5 / React 19.2 / Vite 7.3 / TS 5.8.
- **Tailwind v4 + shadcn/ui**: `@tailwindcss/vite` 플러그인, `src/index.css` 에 토큰 정의(Phase 3 에서 전용 토큰으로 교체 예정), `@/*` 경로 별칭, Button 컴포넌트 추가.
- **테스트·린트**: Vitest(jsdom + Testing Library) / ESLint flat config + Prettier / rustfmt + clippy `-D warnings`. 문서(`*.md`)는 Prettier 대상에서 제외 — 도구가 관리하는 AGENTS.md 훼손 방지.
- **코어(Rust, Tauri 비의존)**
  - `core::file_type` — 매직 바이트 1순위 + 확장자 보조. HWPX 는 ZIP 첫 엔트리 `mimetype`(`application/hwp+zip`)으로 확정하고, HWP 는 OLE 컨테이너 + 확장자로 좁힌다. 확장자가 매직과 어긋나면 매직을 신뢰하고 `extension_mismatch` 로 알린다.
  - `core::job` — 스레드 공유 작업 큐. `Queued→Running→Cancelling→Cancelled/Completed/Failed` 상태 기계. 실행 중 취소는 즉시 죽이지 않고 워커가 정리 후 확정한다.
  - `core::events` — `EventSink` 트레이트 뒤로 Tauri 를 감춰 이벤트 흐름을 런타임 없이 테스트한다. 같은 진행률 재보고는 이벤트를 중복 발행하지 않고, sink 발행 실패는 삼키지 않고 `ReportError::Emit` 으로 돌려준다.
- **셸**: `TauriEventSink`(AppHandle::emit) + 커맨드 `start_demo_job` / `cancel_job` / `list_jobs`.
- **프론트**: `lib/jobs.ts`(타입 있는 이벤트 구독·커맨드 래퍼)와 `JobProgressDemo`(진행바·취소·에러 표시).
- **CI**: `ci.yml`(프론트 lint/typecheck/vitest + 코어 fmt/clippy/test 를 macOS·Windows 매트릭스), `build.yml`(태그·수동 실행에서 dmg/nsis 아티팩트).

## 동작 흐름

1. 프론트가 `start_demo_job` 호출 → 셸이 큐에 등록하고 `claim_next` 로 Running 전환, 워커 스레드 기동.
2. 워커가 100ms 마다 5%씩 `report_progress` → `JobReporter` 가 큐 상태를 갱신하고 `job://event` 로 발행.
3. `subscribeToJobEvents` 가 payload 를 꺼내 React 상태로 반영 → 진행바·퍼센트 갱신.
4. 취소를 누르면 `cancel_job` → 큐가 `Cancelling` 으로 바뀌고, 워커가 다음 틱에서 이를 보고 `mark_cancelled` 로 확정한다.

## 검증

- `pnpm lint`·`pnpm typecheck`·`pnpm build` 통과, Vitest 17개 / `cargo test` 38개 전부 그린, `cargo fmt --check`·`cargo clippy -- -D warnings` 경고 0.
- `pnpm tauri dev` 로 앱 창을 띄워 데모 카운터를 실제 클릭 — 0% 대기 → 50% 변환 중(취소 버튼 노출) → 100% 완료까지 스크린샷으로 확인했다.
- CI 워크플로는 아직 푸시 전이라 GitHub Actions 그린 여부는 미확인.

## 메모

`#ci-test`·`#ci-build` 는 파일만 작성한 상태 — 푸시 후 Actions 결과를 봐야 완료 처리할 수 있다.