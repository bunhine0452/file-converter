---
schema_version: 1
type: bug
slug: "preflight-note-dropped-before-ui"
status: done
difficulty: medium
created_at: "2026-08-07T20:22:16+09:00"
session_id: "mcp-20260807-202216"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "src-tauri/src/core/events.rs"
    op: update
  - path: "src-tauri/src/shell/runtime_manager.rs"
    op: update
  - path: "src-tauri/src/shell/commands.rs"
    op: update
  - path: "src-tauri/src/core/soffice/runner.rs"
    op: update
  - path: "src-tauri/src/core/runtime/real_installer.rs"
    op: update
  - path: "src-tauri/examples/verify_runtime.rs"
    op: update
  - path: "src/lib/jobs.ts"
    op: update
  - path: "src/hooks/useConversionQueue.ts"
    op: update
  - path: "src/hooks/useConversionQueue.test.ts"
    op: update
  - path: "src/components/ConversionQueue.tsx"
    op: update
  - path: "src/components/ConversionQueue.test.tsx"
    op: create
related: []
tags:
  - "phase2"
  - "hwp"
  - "preflight"
  - "배포용문서"
  - "hwpx"
  - "tdd"
  - "ux"
  - "mcp-tool"
---
[x] 프리플라이트 안내가 변환 경계에서 버려져 사용자에게 닿지 않았다

`hwp-errors` 의 남은 구멍을 코드에서 찾아 메웠다. 암호·DRM 거부 경로는 이미 살아 있었지만, **거부하지 않고 통과시키는 쪽**의 안내는 계산만 하고 버려지고 있었다.

## 발생 원인

`classify_hwp5` 는 배포용(읽기 전용) 문서를 `Preflight::ProceedWithNote(NOTE_DISTRIBUTABLE)` 로 판정하고, HWPX manifest 에 암호화 항목이 있으면 `NOTE_HWPX_ENCRYPTION_DATA` 를 붙인다. 순수 함수 단위로는 테스트까지 갖춰져 있었다.

그런데 `RuntimeManager::convert_to_pdf` 의 프리플라이트 분기가 이랬다:

```rust
Preflight::Proceed | Preflight::ProceedWithNote(_) => {}
```

안내를 패턴에서 **버리고** 있었다. 반환형이 `Result<(), String>` 이라 담을 자리도 없었다. 결과적으로 배포용 문서를 넣은 사용자는 서식이 원본과 다를 수 있다는 경고를 한 번도 못 본 채 PDF 를 받았다 — 프리플라이트가 존재하는 이유(빈 PDF·틀어진 결과를 말없이 넘기지 않는다)의 절반이 UI 직전에서 새고 있었다.

거부 경로만 눈에 띄어서 통과 경로가 방치된 전형적인 형태다. 순수 함수 테스트가 그린이라 더 안 보였다.

## 해결 방법

안내가 코어에서 화면까지 끊기지 않게 경로를 새로 이었다. TDD 로 세 단위를 각각 RED 확인 후 구현했다.

1. **이벤트** — `JobEvent::Note { id, message }` 와 `JobReporter::note()` 추가. 안내는 순수 통지라 상태·진행률을 건드리지 않는다(그러면 UI 가 실패로 오해한다). 없는 id 는 `NotFound` 에러이고 이벤트를 남기지 않는다.
2. **변환 경계** — `convert_to_pdf` 반환형을 `Result<Option<&'static str>, String>` 으로 바꿔 안내를 실어 보낸다. `commands.rs` 는 `complete` **앞에** 안내를 발행한다(완료를 보고 UI 가 항목을 접을 수 있으므로).
3. **프론트** — `JobEvent` 유니온에 `note` 추가, `ConversionItem.note` 필드 신설(실패 메시지와 분리 — 한 항목이 둘 다 가질 수 있다), `ConversionQueue` 가 `role="status"` 폴라이트 라이브 리전으로 렌더한다. 에러 색(`text-destructive`)을 입히지 않아 "진행됨"과 "실패"가 시각적으로 구분된다.

곁가지로 `FakeRunner::on_run` 훅이 `&ProcessRequest` 를 받도록 고쳤다. 인자를 못 보면 실제 soffice 가 `--outdir` 아래에 산출물을 만드는 동작을 흉내낼 수 없어, `convert_to_pdf` 전체 경로를 단위 테스트할 방법이 없었다. 이 덕에 그동안 테스트가 하나도 없던 `convert_to_pdf` 에 처음으로 커버리지가 붙었다.

## 검증

- `cargo test` 234 → 242, Vitest 50 → 58 그린. 새 테스트는 전부 먼저 RED 를 눈으로 확인했다(이벤트 6건 컴파일 에러, 변환 `expected (), found Option<_>`, 프론트 4 fail + 컴포넌트 3 fail).
- `cargo clippy --all-targets -- -D warnings` 경고 0, `cargo fmt --check`·`prettier --check`·`tsc --noEmit`·`eslint` 모두 통과.
- 실환경 검증 스크립트(`examples/verify_runtime.rs convert`)도 안내를 출력하도록 맞췄다 — 손으로 돌릴 때 이 경로가 다시 새는지 바로 보인다.

## 메모

실제 배포용 `.hwp` 샘플로 앱 창에서 눈으로 확인한 것은 아직이다(합성 HWP5 헤더로 플래그 조합을 만들어 검증했다). 앞 일지가 남긴 "드래그&드롭 UI 손 검증"·대용량 시나리오는 그대로 남아 있다.

교훈: 거부 분기만 검증하면 통과 분기의 부수 정보는 조용히 사라진다. `_` 로 버리는 패턴은 그 자체가 냄새였다.