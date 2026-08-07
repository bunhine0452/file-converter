---
schema_version: 1
type: feature
slug: "phase2-core-soffice-hwp-preflight"
status: done
difficulty: high
created_at: "2026-08-07T17:34:50+09:00"
session_id: "mcp-20260807-173450"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "src-tauri/src/core/fs_port.rs"
    op: create
  - path: "src-tauri/src/core/soffice/probe.rs"
    op: create
  - path: "src-tauri/src/core/soffice/runner.rs"
    op: create
  - path: "src-tauri/src/core/soffice/profile.rs"
    op: create
  - path: "src-tauri/src/core/soffice/version.rs"
    op: create
  - path: "src-tauri/src/core/soffice/detect.rs"
    op: create
  - path: "src-tauri/src/core/soffice/invoke.rs"
    op: create
  - path: "src-tauri/src/core/soffice/outcome.rs"
    op: create
  - path: "src-tauri/src/core/hwp/preflight.rs"
    op: create
  - path: "src-tauri/src/core/hwp/message.rs"
    op: create
  - path: "src-tauri/src/core/runtime/assets.rs"
    op: create
  - path: "src-tauri/src/core/runtime/download.rs"
    op: create
  - path: "src-tauri/src/core/runtime/plan.rs"
    op: create
  - path: "src/hooks/useFileDrop.ts"
    op: create
  - path: "src/components/Dropzone.tsx"
    op: create
  - path: "src/test/tauri.ts"
    op: create
  - path: "src-tauri/capabilities/default.json"
    op: update
  - path: "THIRD-PARTY-NOTICES.md"
    op: update
related: []
tags:
  - "phase2"
  - "hwp"
  - "libreoffice"
  - "tdd"
  - "tauri"
  - "mcp-tool"
---
[x] Phase 2 코어 — soffice 탐지·HWP 프리플라이트·변환 판정·설치 계획

## 추가 기능

Phase 2(HWP/HWPX→PDF)의 **코어 로직 전체**를 TDD로 구현했다. 실제 변환 실행(셸 배선·설치 실행)은 다음 단계.

- `core/soffice/` — 탐지(`detect`), 전용 프로필 URL(`profile`), 버전 계약(`version`), argv 조립(`invoke`), 성공/실패 판정(`outcome`)
- `core/hwp/` — HWP5 헤더 프리플라이트(`preflight`)와 한국어 메시지(`message`)
- `core/runtime/` — 자산 pin(`assets`), 해시 검증 다운로더(`download`), 설치 계획(`plan`)
- 프론트 — `useFileDrop`(Tauri 네이티브 드롭), `Dropzone`, `test/tauri.ts`(IPC 목 헬퍼)

부작용은 `FileSystem`/`SofficeProbe`/`ProcessRunner`/`Downloader` 4개 트레이트 뒤로 밀어, **LibreOffice가 없는 개발 머신에서 전부 단위 테스트**된다.

## 동작 흐름

리서치(웹 조사 5주제 → 반박 검증 → 스펙 합성)로 확정한 사실을 코드로 옮겼다. 특히 세 가지가 설계를 결정했다.

1. **암호 HWP는 조용히 빈 PDF를 만든다.** H2Orestart의 `impl_import()`는 `HwpParseException`을 catch하고도 return하지 않고 빈 섹션으로 진행해 무조건 성공을 반환한다. → soffice를 띄우기 **전에** FileHeader 플래그로 거르는 프리플라이트가 유일한 방어. `#hwp-errors`를 `#hwp-happy`보다 먼저 만든 이유다.
2. **exit 0을 성공으로 믿을 수 없다.** LibreOffice 26.2 미만은 변환에 실패해도 EXIT_SUCCESS를 낸다(tdf#148275). → 타임아웃 → stderr 패턴 → 종료 코드 → 산출물(존재·`%PDF-` 매직·크기) 4단계 복합 판정. 반대로 exit 139(SIGSEGV)라도 PDF가 멀쩡하면 통과시킨다. 단 exit 77(기동 실패)은 그 변경 이전부터 있던 규약이라 버전과 무관하게 신뢰한다.
3. **JRE가 확장보다 먼저다.** JRE 없이 soffice 프로필이 한 번 초기화되면 이후 `JAVA_HOME`을 올바로 줘도 계속 `source file could not be loaded`로 실패하고, 복구 수단은 프로필 삭제뿐이다. → 설치 계획이 순서를 코드로 강제하고 `ResetProfile` 단계를 둔다.

그 밖에 코드에 박아둔 함정들: `--infilter`는 타입명이라 무시되고 SIGABRT 보고가 있어 넣지 않는다 / Windows는 `soffice.exe`가 stdout을 캡처할 수 없어 `.com` 우선 / macOS GUI 앱은 로그인 셸 PATH를 상속받지 않아 PATH 탐색을 마지막에 둔다 / 32비트 LibreOffice가 아직 배포돼 WOW64 32비트 레지스트리 뷰를 유지한다 / `unopkg --shared`는 관리자 권한을 요구해 쓰지 않는다 / Windows 로밍 프로필에는 런타임을 설치하지 않는다.

병렬 구현은 격리된 git worktree 4개에서 진행했고, 산출물을 메인 트리로 통합한 뒤 핵심 사실(자산 해시 9개, argv, 우선순위, 판정 순서)을 직접 대조 검증했다.

## 검증

- `cargo test` 173개 · Vitest 37개 그린, `clippy -D warnings` 경고 0, lint·typecheck 통과
- GitHub Actions CI가 macOS·Windows 양쪽에서 그린 (PR #2)
- 첫 Windows 실행에서 경로 구분자 문제로 탐지 테스트 2개가 깨져, 비교 전 정규화로 수정

## 메모

실제 변환은 아직 확인 못 했다 — 이 머신에 LibreOffice도 HWP 샘플도 없다. 실환경에서 확인해야 할 것: `unopkg`가 `-env:UserInstallation`을 존중하는지(전용 프로필 설계의 근간), 신규 프로필에서 `JAVA_HOME` 주입만으로 Java가 인식되는지, 암호 HWP의 실제 증상이 소스 추론과 같은지, Windows `msiexec /a`가 UAC 없이 동작하는지.