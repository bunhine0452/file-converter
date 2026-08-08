---
schema_version: 1
type: bug
slug: "extension-query-scope-too-narrow"
status: done
difficulty: medium
created_at: "2026-08-08T20:40:42+09:00"
session_id: "mcp-20260808-204042"
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
  - path: "src-tauri/src/core/soffice/runner.rs"
    op: update
related: []
tags:
  - "phase2"
  - "libreoffice"
  - "h2orestart"
  - "unopkg"
  - "실환경"
  - "tdd"
  - "회귀"
  - "mcp-tool"
---
[x] 번들 스코프만 조회해 등록된 확장을 "없다"고 단정했다

대용량 검증을 하려고 `verify_runtime status` 를 돌렸다가 `extension: NotRegistered` 를 봤다. 변환은 멀쩡히 되는데 상태만 "확장을 설치해야 합니다"였다 — 어제 `#lo-h2o` 를 완료로 닫은 그 자리다.

## 발생 원인

어제 커밋(8c1b58f)은 "번들 디렉토리에 넣은 확장은 옵션 없는 `unopkg list` 에 안 보인다"는 전제로 조회 스코프를 설치 전략에 맞췄다 — 앱이 설치한 LibreOffice 면 `list --bundled`.

실환경에서 직접 두 스코프를 돌려 보니 전제가 반대였다.

```
unopkg list --bundled  → dict-fr / hunspell / NLPSolver ... (H2Orestart 없음)
unopkg list            → Identifier: ebandal.libreoffice.H2Orestart
                          Version: 0.7.13, is registered: yes
```

**확장을 넣는 곳과 LibreOffice 가 등록해 두는 곳이 다르다.** 번들 확장 디렉토리에 푼 `H2Orestart` 는 기동할 때(우리 `warm_up_profile`) 스캔돼 **전용 프로필(user)** 에 등록된다. `--bundled` 목록은 LibreOffice 가 자체 등록 DB 로 들고 온 확장만 나열하므로 우리 확장은 영영 뜨지 않는다.

그래서 상태는 항상 `needsExtension` — 사용자는 이미 되는 변환을 두고 설치 버튼을 계속 권유받고, `verify_extension` 도 같은 이유로 설치 완료 직후 실패한다.

(조사 도중 딴 길로 샜던 것 하나: 손으로 돌린 `unopkg` 가 DeploymentException 을 뱉길래 `Application Support` 의 공백이 원인인가 싶었는데, `ProfileUrl` 은 이미 퍼센트 인코딩을 하고 있었다 — 인코딩을 빠뜨린 건 내 손 명령 쪽이었다. 앱 코드는 무죄.)

## 해결 방법

한쪽만 믿지 않는다. 설치 전략 스코프를 먼저 보고, 거기서 못 찾으면 반대 스코프도 본다.

- `other_scope(strategy)` — 두 스코프가 서로를 가리킨다.
- `merge_extension_states(primary, fallback)` — 어느 쪽이든 Registered 면 등록이다. 조회 실패(`Unknown`)를 "미등록"으로 덮지 않는다(그러면 재설치를 무한히 권한다). 읽어낸 쪽이 하나라도 있으면 그쪽을 믿는다.
- 테스트를 위해 `FakeRunner::responding_with` 를 추가했다 — 같은 프로그램을 인자(`--bundled`)만 바꿔 두 번 부르는 흐름은 프로그램 경로별 고정 응답으로는 흉내낼 수 없었다.

## 검증

- 실패 재현 테스트 먼저: "번들 스코프에 없어도 사용자 스코프에 있으면 등록이다" 가 `NotRegistered` 로 RED → 구현 후 GREEN. 순수 병합 규칙 4건도 RED 확인 후 구현.
- `cargo test` 261 그린, clippy·fmt 통과.
- 실환경 `verify_runtime status` 가 `extension: Registered { version: "0.7.13" }` 로 복귀. 같은 런타임으로 105MB 변환도 성공.

## 메모

교훈: "실환경에서 확인했다"고 적은 항목이 하루 만에 뒤집혔다. 어제 관측은 한 스코프만 보고 내린 결론이었고, 반대 스코프를 같이 찍어 봤다면 바로 드러났을 일이다. 외부 도구의 출력은 **한 번의 성공이 아니라 양쪽을 다 찍어 봐야** 전제가 선다.