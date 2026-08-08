---
schema_version: 1
type: chore
slug: "verify-build-matrix-artifacts"
status: done
difficulty: verylow
created_at: "2026-08-08T21:07:10+09:00"
session_id: "mcp-20260808-210710"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched: []
related: []
tags:
  - "phase1"
  - "ci"
  - "릴리스"
  - "검증"
  - "mcp-tool"
---
[x] mac/win 번들 매트릭스를 처음 돌려 설치본 아티팩트를 확인했다

`ci-build` 은 워크플로만 써 두고 한 번도 돌린 적이 없어 `~` 로 남아 있었다. Build 는 태그 푸시와 수동 실행에서만 도는 설정이라 지금까지 실행 이력이 0 이었다.

## 한 일

오늘 커밋 3건을 `origin/main` 에 푸시하고 `gh workflow run build.yml` 로 매트릭스를 수동 실행했다.

## 검증

- 실행 31256003000 — 9분 23초, 두 잡 모두 성공
  - `mac (aarch64) — dmg`: success
  - `win (x64) — nsis`: success
- 아티팩트 실물 확인
  - `file-converter-aarch64-apple-darwin` 4MB
  - `file-converter-x86_64-pc-windows-msvc` 3MB
- 같은 푸시로 돈 CI(lint+Vitest+cargo test)도 1분 48초 그린.

## 메모

`if-no-files-found: error` 를 걸어 둔 덕에 "빌드는 됐는데 번들이 비었다"는 조용한 실패는 애초에 불가능하다. 다만 아직 서명·공증은 없다 — 내려받아 실행하면 mac 은 Gatekeeper, win 은 SmartScreen 경고가 뜬다(#sign-mac, #sign-win 에서 다룬다).