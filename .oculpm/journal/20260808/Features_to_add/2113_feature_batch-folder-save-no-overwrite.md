---
schema_version: 1
type: feature
slug: "batch-folder-save-no-overwrite"
status: done
difficulty: low
created_at: "2026-08-08T21:13:04+09:00"
session_id: "mcp-20260808-211304"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "src-tauri/src/core/output.rs"
    op: create
  - path: "src-tauri/src/core/mod.rs"
    op: update
  - path: "src-tauri/src/shell/commands.rs"
    op: update
  - path: "src-tauri/src/lib.rs"
    op: update
  - path: "src/lib/runtime.ts"
    op: update
  - path: "src/App.tsx"
    op: update
  - path: "src/App.test.tsx"
    op: update
related: []
tags:
  - "phase2"
  - "phase3"
  - "ux"
  - "일괄변환"
  - "데이터안전"
  - "tdd"
  - "mcp-tool"
---
[x] 일괄 변환은 폴더를 한 번만 묻고 남의 파일을 덮어쓰지 않는다

UX 향상 요청에 따라 남아 있던 가장 큰 마찰을 걷어냈다.

## 추가 기능

드롭한 파일 **하나하나마다** 저장 대화상자를 띄우고 있었다. 10개를 드롭하면 10번 답해야 끝난다 — 일괄 변환이라 부를 수 없는 흐름이었다.

- **한 건**: 지금처럼 저장 위치를 묻는다. 사용자가 경로와 (필요하면) 덮어쓰기에 동의하는 자리다.
- **여러 건**: 폴더를 **한 번만** 묻고 파일명은 코어가 정한다.

## 동작 흐름

폴더만 고른 경우 사용자는 개별 파일 이름에 동의한 적이 없다. 그래서 `core/output.rs` 가 겹치지 않는 이름을 만든다.

- `pdf_name_for("보고서.v2.hwp")` → `보고서.v2.pdf` (중간 점 보존, 대문자 확장자도 소문자 `.pdf`)
- `unique_output_path` — 이미 있으면 `보고서 (1).pdf`, 또 있으면 `(2)` … 1000번까지 시도하고 그래도 겹치면 나노초를 붙여 반드시 반환한다(무한 루프 금지).

프론트는 `plan_output_path` 커맨드로 최종 경로를 받아 큐에 그 경로를 기록한다 — 목록의 "PDF 열기"가 실제 저장된 파일을 연다.

## 검증

- RED 먼저: 3개 드롭에 `plugin:dialog|save` 가 3번 불리는 것을 확인한 뒤 폴더 1회 + `plan_output_path` 2회로 바뀌는 것을 구현.
- 경로 규칙 단위 테스트 6건(빈 폴더·1회 충돌·2회 충돌·중간 점·대문자 확장자·전부 충돌).
- `cargo test` 271 · Vitest 91 그린. clippy 0 · fmt · tsc · eslint · prettier 통과.

## 메모

"말없이 덮어쓰지 않는다"는 규칙은 저장 대화상자 경로에는 적용하지 않았다 — 거기서는 OS 대화상자가 이미 덮어쓰기를 물어보고 사용자가 답한다. 동의를 받은 곳과 받지 않은 곳을 구분하는 게 요점이다.