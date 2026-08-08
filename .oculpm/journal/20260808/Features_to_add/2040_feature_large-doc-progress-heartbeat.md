---
schema_version: 1
type: feature
slug: "large-doc-progress-heartbeat"
status: done
difficulty: medium
created_at: "2026-08-08T20:40:07+09:00"
session_id: "mcp-20260808-204007"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "src-tauri/src/core/progress.rs"
    op: create
  - path: "src-tauri/src/core/mod.rs"
    op: update
  - path: "src-tauri/src/core/events.rs"
    op: update
  - path: "src-tauri/src/shell/commands.rs"
    op: update
  - path: "src-tauri/src/shell/runtime_manager.rs"
    op: update
  - path: "src-tauri/examples/verify_runtime.rs"
    op: update
  - path: "src/hooks/useConversionQueue.ts"
    op: update
  - path: "src/hooks/useConversionQueue.test.ts"
    op: update
related: []
tags:
  - "phase2"
  - "hwp"
  - "progress"
  - "대용량"
  - "tdd"
  - "ux"
  - "tauri"
  - "mcp-tool"
---
[x] 대용량 변환에서 진행 표시가 멈추지 않게 하트비트를 달았다

`hwp-large` — 100MB급 문서가 UI 멈춤 없이, 진행 표시를 유지한 채 변환되게 했다.

## 추가 기능

soffice 는 변환 중 진행 정보를 한 줄도 주지 않는다. 그래서 지금까지는 시작할 때 5% 를 한 번 보내고 끝날 때까지 침묵했다 — 105MB 문서에서는 막대가 2분 가까이 5% 에 붙어 있어 앱이 죽은 것으로 보인다.

`core/progress.rs` 를 새로 만들어 "얼마나 걸릴 것 같은가"로 추정 진행률을 흘린다.

- `heartbeat_percent(경과, 예상)` — 5% 에서 출발해 예상 시간에 95% 까지 선형으로 차오르고, 예상을 넘기면 95% 에 머문다. **추정은 절대 100% 에 닿지 않는다** (100% 는 실제 완료만 쓴다).
- `expected_duration(제한시간) = 제한시간/2` — 크기 비례 규칙은 `timeout_for` 하나만 알고 있게 두었다. 두 곳에서 따로 계산하면 막대가 다 찬 뒤에도 한참 남거나, 반도 못 찬 채 타임아웃이 난다.
- `Heartbeat` — Condvar 기반 배경 스레드. 1초마다 경과 시간을 알리고, 정지 신호는 즉시 받는다.

## 동작 흐름

`convert_hwp` 워커 스레드가 시작 5% 를 보낸 뒤 하트비트를 걸고, 변환이 끝나면 **완료·실패보다 먼저** 하트비트를 정지(join)한다.

늦게 도착한 추정치가 결과를 덮는 경로를 두 겹으로 막았다.

1. 코어 — `report_progress` 가 Queued/Running 이 아닌 작업의 진행률을 무시한다. 취소를 눌렀는데 막대가 다시 오르면 사용자는 취소가 씹힌 줄 안다.
2. 프론트 — `applyEvent` 가 완료·실패·취소(중) 항목에 온 progress 이벤트를 버린다.

곁가지로 `get_runtime_status` 를 async + `spawn_blocking` 으로 바꿨다. 상태 조회는 변환과 같은 프로필 잠금을 기다리는데, 동기 커맨드는 메인 스레드에서 돌기 때문에 대용량 변환 중 상태를 한 번 조회하면 그동안 창이 통째로 얼어붙었다 — "UI 멈춤 없이"의 진짜 구멍은 변환이 아니라 여기였다.

## 검증

- `cargo test` 242 → 261, Vitest 58 → 61 그린. 새 테스트는 전부 먼저 RED 를 확인했다. 특히 "정지는 다음 간격을 기다리지 않는다" 가 실제 미스드 노티피케이션 경합을 잡아냈다(정지 신호가 wait 진입 전에 도착하면 30초를 매달렸다) — `wait_timeout_while` 로 고쳤다.
- 실환경(실물 .hwpx 를 본문 60배로 부풀리고 100MB 패딩을 넣은 105MB 문서): **105.9초에 1621쪽 PDF 성공**, 1초마다 5%→34% 로 갱신되는 것을 `verify_runtime convert` 로 확인. 참고 측정 — 830KB/27쪽 10.1초, 2MB/540쪽 23.1초.
- `cargo clippy --all-targets -- -D warnings`·`cargo fmt --check`·`tsc --noEmit`·`eslint`·`prettier --check` 통과.

## 메모

예상 시간을 제한 시간의 절반으로 잡으면 실제 완료 시점의 막대는 34~74% 사이였다(측정 3건). 끝나기 전에 100% 를 찍는 거짓말보다는 낫다고 보고 그대로 뒀다 — 실측이 더 쌓이면 계수를 조정할 자리다.

앱 창에서 실제로 드래그&드롭해 얼어붙지 않는지 눈으로 보는 것은 아직 남았다(하네스로만 검증).