---
schema_version: 1
type: feature
slug: "phase2-runtime-install-and-ui"
status: done
difficulty: high
created_at: "2026-08-07T18:32:25+09:00"
session_id: "mcp-20260807-183225"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "src-tauri/src/core/runtime/installer.rs"
    op: create
  - path: "src-tauri/src/core/runtime/real_installer.rs"
    op: create
  - path: "src-tauri/src/core/hwp/inspect.rs"
    op: create
  - path: "src-tauri/src/shell/runtime_manager.rs"
    op: create
  - path: "src-tauri/src/shell/commands.rs"
    op: update
  - path: "src-tauri/src/shell/mod.rs"
    op: update
  - path: "src-tauri/src/lib.rs"
    op: update
  - path: "src/lib/runtime.ts"
    op: create
  - path: "src/components/RuntimeStatus.tsx"
    op: create
  - path: "src/hooks/useConversionQueue.ts"
    op: create
  - path: "src/components/ConversionQueue.tsx"
    op: create
  - path: "src/App.tsx"
    op: update
  - path: "src/components/JobProgressDemo.tsx"
    op: delete
related: []
tags:
  - "phase2"
  - "libreoffice"
  - "installer"
  - "tauri"
  - "ui"
  - "mcp-tool"
---
[x] 런타임 온디맨드 설치 실행기와 변환 화면 연결

## 추가 기능

Phase 2 코어(PR #3) 위에 **실제 설치 실행 + 셸 배선 + 화면**을 얹어, 드롭부터 변환 목록까지 흐름을 이었다.

- `core/runtime/installer` + `real_installer` — dmg/msi/tar.gz/zip/oxt 설치. OS 도구(hdiutil·ditto·xattr·msiexec·tar)를 외부 프로세스로만 부르고, 경로 규칙·argv 는 순수 함수로 분리했다.
- `core/hwp/inspect` — 실제 파일을 열어 프리플라이트한다 (CFB `FileHeader` / ZIP `mimetype`·`manifest`).
- `shell/runtime_manager` — 상태 조회·설치·변환을 한 곳에서 직렬화.
- 커맨드 `get_runtime_status` / `install_runtime` / `convert_hwp`.
- 화면 `RuntimeStatus`(설치 상태·진행 막대) + `useConversionQueue` + `ConversionQueue`, App 이 드롭→저장 위치→변환→목록을 잇는다.
- 역할을 다한 데모(`JobProgressDemo`, `start_demo_job`) 제거.

## 동작 흐름

설계에서 붙잡은 함정들:

1. **설치가 끝났다고 믿지 않는다.** `soffice` 실행 파일과 `JAVA_HOME/bin/java` 존재를 확인한다. 빈 디렉토리를 JAVA_HOME 으로 넘기면 나중에 soffice 가 `source file could not be loaded` 로 애매하게 실패한다. 테스트가 실제로 이 구멍을 잡아 구현을 고쳤다.
2. **복사가 실패해도 dmg 는 반드시 뗀다.** 마운트가 남으면 다음 설치가 막힌다.
3. **`ditto` + quarantine 제거.** `cp` 로는 리소스 포크·권한이 깨지고, 격리 속성이 남으면 Gatekeeper 가 실행을 막는다.
4. **`TARGETDIR=` 는 값과 붙인 인자 하나.** 쪼개면 공백 있는 경로가 깨진다.
5. **이벤트 유실 방어.** 커맨드가 작업 id 를 돌려주기 전에 이벤트가 먼저 도착할 수 있다 — 암호 문서는 프리플라이트에서 즉시 실패하므로 실제로 발생한다. 프론트 훅이 미등록 id 의 이벤트를 버퍼링했다가 등록 직후 적용한다.
6. **드롭 경로에는 디렉토리가 섞여 온다.** `convert_hwp` 가 Rust 에서 `is_file` 을 다시 본다.

테스트 페이크도 함께 키웠다. `FakeRunner` 에 부작용 훅을 넣어 "도구가 파일을 만들어낸다"를 모델링하고, `FakeFs` 에 실행 중 파일을 추가하는 수단을 뒀다.

## 검증

- `cargo test` 215개 · Vitest 50개 그린, clippy 경고 0, lint·typecheck·build 통과
- CI 가 macOS·Windows 양쪽에서 그린 (PR #4, main 에 머지)
- Windows 러너에서 두 번 깨졌다: 경로 구분자 문자열 비교(PR #3), `ProfileUrl` 이 호스트 규칙으로 절대 경로를 요구하는데 테스트가 유닉스 경로를 고정으로 넣은 것(PR #4). 둘 다 테스트 쪽 결함이었다.

## 메모

실제 변환은 아직 확인 못 했다. `install_runtime` 을 한 번 돌리면(LibreOffice 297MB + JRE 48MB + 확장 0.6MB) 네 가지가 한꺼번에 확인된다 — unopkg 가 `-env:UserInstallation` 을 존중하는지, 신규 프로필에서 JAVA_HOME 주입만으로 Java 가 인식되는지, dmg→ditto→quarantine 경로가 실제로 도는지, 암호 HWP 의 실제 증상이 소스 추론과 같은지.

작업 중 앱 창 스크린샷을 찍으려다 화면 영역 캡처가 앞에 있던 에디터의 `.env.local` 을 대신 담았다. 파일은 즉시 삭제하고 사용자에게 알렸다. `screencapture -R` 는 창이 아니라 화면 좌표를 찍으므로 이 방식은 다시 쓰지 않는다.