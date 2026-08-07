---
schema_version: 1
type: bug
slug: "h2orestart-verify-scope-mismatch"
status: done
difficulty: high
created_at: "2026-08-07T19:29:08+09:00"
session_id: "mcp-20260807-192908"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "src-tauri/src/core/runtime/plan.rs"
    op: update
  - path: "src-tauri/src/shell/runtime_manager.rs"
    op: update
related: []
tags:
  - "phase2"
  - "libreoffice"
  - "h2orestart"
  - "unopkg"
  - "macos"
  - "실환경검증"
  - "tdd"
  - "mcp-tool"
---
[x] 앞 일지가 남긴 "macOS unopkg 블로커"를 재현해 보니 블로커가 아니었다

## 발생 원인

앞 일지는 `unopkg add` 의 `NoConnectException` 을 남은 블로커로 적고, "번들 디렉토리에 풀어둔 것은 `unopkg list --bundled` 에 아예 잡히지 않는다"고 기록했다. 실환경에서 직접 확인하니 **번들 확장은 완전히 등록돼 있었다** — `H2Orestart.jar` 까지 `is registered: yes`. 앞 기록이 틀렸다.

진짜 결함은 조회 쪽이었다.

1. **확장 조회 스코프가 설치 전략과 어긋났다** — 앱이 설치한 LibreOffice 에는 번들 디렉토리(`Contents/Resources/extensions/`)에 확장을 푸는데(`ExtensionStrategy::BundledDir`), 상태 조회와 `VerifyExtension` 은 옵션 없는 `unopkg list` 를 썼다. 이 명령은 **사용자 확장만** 나열한다. 그래서 설치가 실제로 성공해도 검증은 "확장이 등록되지 않았습니다"로 실패하고, 다음 실행에서 계획이 확장 설치를 처음부터 다시 잡는다. 무한 재설치.

2. **하위 패키지 판정이 우연에 기대고 있었다** — 한 확장 블록에는 `is registered:` 가 여러 번 나온다. 첫 줄이 확장 자체, `bundled Packages` 안의 나머지는 하위 패키지(jar·rdb·xcu)다. 파서는 마지막 줄로 덮어써서 판정했다. 확장은 `yes` 인데 `H2Orestart.jar` 만 꺼진 상태 — HWP 필터가 동작하지 않는 바로 그 상태 — 를 뒤따르는 `.xcu` 가 `yes` 면 "등록됨"으로 보고했다.

`NoConnectException` 자체는 실재하지만 macOS `unopkg add` 경로에서만 나고, 앱이 설치한 LibreOffice 는 그 경로를 쓰지 않는다. 기존 주석이 이미 그렇게 적고 있었다.

## 해결 방법

- `unopkg_list_args` 가 `ExtensionStrategy` 를 받아 `BundledDir` 이면 `--bundled` 를 붙인다. 어디에 넣을지와 어디서 찾을지가 갈라지지 않게 `extension_strategy_for(managed: bool)` 하나로 결정을 모으고, 계획 수립과 매니저가 함께 쓴다.
- `parse_unopkg_list` 는 확장 자체의 첫 `is registered:` 와 하위 패키지들을 구분한다. **하나라도 미등록이면 등록으로 보지 않는다** — 우연히 맞던 동작을 의도한 규칙으로 바꿨다. `unknown` 도 등록이 아니다.
- 실제 캡처한 `unopkg list --bundled` 출력(설명문에 URL·한글 포함) 전체를 회귀 앵커 테스트로 박았다.

TDD 로 진행했다: 실패 3건을 눈으로 확인한 뒤 구현 (cargo test 225→234).

## 검증

- `cargo test` 234 / Vitest 50 그린, `cargo clippy --all-targets -- -D warnings` 경고 0, `cargo fmt --check` 통과
- 실환경 상태가 `extension: Registered { version: "0.7.13" }` 으로 바뀌고, 재실행 시 설치 계획이 비어 멱등하다
- **실제 문서 2건이 PDF 로 변환됐다** — `.hwp` → 7쪽, `.hwpx` → 27쪽, 둘 다 `%PDF-1.7`. 산출물 1쪽을 눈으로 대조해 한글 본문·표 구조·셀 음영이 깨짐 없이 재현됨을 확인했다.

## 메모

남은 것: 앱 창에 실제로 드래그&드롭하는 UI 경로는 아직 손으로 돌려보지 않았다 (변환 코어는 실물 샘플로 증명됨). 대용량·암호 문서 시나리오도 그대로 남아 있다.

앞선 실패 경로가 남긴 반쯤 등록된 *사용자* 확장이 프로필에 남아 있지만, 번들 스코프로 조회하므로 무시되고 변환에도 영향이 없었다. 새 설치에서는 생기지 않는다.

교훈: 일지의 "블로커"도 재현부터 한다. 이번엔 앞 기록의 단정 하나가 방향을 통째로 틀어놓을 뻔했다.