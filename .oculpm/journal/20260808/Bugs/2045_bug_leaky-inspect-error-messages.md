---
schema_version: 1
type: bug
slug: "leaky-inspect-error-messages"
status: done
difficulty: low
created_at: "2026-08-08T20:45:23+09:00"
session_id: "mcp-20260808-204523"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "src-tauri/src/core/hwp/message.rs"
    op: update
  - path: "src-tauri/src/shell/runtime_manager.rs"
    op: update
related: []
tags:
  - "phase2"
  - "hwp"
  - "에러처리"
  - "ux"
  - "실샘플"
  - "tdd"
  - "mcp-tool"
---
[x] 손상 파일 안내에 영문 라이브러리 진단이 그대로 새어 나갔다

`hwp-errors` 의 남은 조건은 "실샘플로 손검증"이었다. 실물 `.hwp` 를 변형해 네 경로를 돌려 보니 두 건이 새고 있었다.

## 발생 원인

`InspectError` 의 Display 가 원인 문자열을 그대로 품고, `convert_to_pdf` 는 그걸 `to_string()` 해서 UI 로 보냈다.

```
잘린 파일   → 문서 구조를 해석하지 못했습니다: DIFAT refers to sector 119, but sector count is only 54
가짜 파일   → 문서 구조를 해석하지 못했습니다: Invalid CFB file (330 bytes is too small)
```

앞부분은 한국어지만 뒤에 `cfb` 크레이트의 영문 진단이 붙는다. 사용자는 "DIFAT" 을 보고 할 수 있는 일이 없고, 두 경우 모두 실제로 해야 할 일("이 파일은 한글 문서가 아니거나 깨졌다")은 안내되지 않았다.

단위 테스트가 `matches!(result, Err(InspectError::Malformed(_)))` 까지만 확인해서 **메시지 내용은 아무도 안 보고 있었다.**

## 해결 방법

`inspect_error_message(&InspectError)` 를 message.rs 에 추가해 두 갈래로 안내한다.

- `Io` → "파일을 열지 못했습니다. 파일이 옮겨졌거나 권한이 없는지 확인해 주세요."
- `Malformed` → "문서를 열지 못했습니다. 한글 문서가 아니거나 파일이 손상됐습니다."

원인 문자열은 `eprintln!` 로그로만 남긴다. 두 경우를 더 잘게 가르려면 영문 진단을 문자열 매칭해야 하는데, 그건 라이브러리 문구가 바뀌는 순간 깨지는 약속이라 하지 않았다.

## 검증

- RED 먼저: "내부 진단이 샌다: 문서 구조를 해석하지 못했습니다: Invalid CFB file (32 bytes is too small)" — 안내에 `CFB`·`sector`·`Invalid` 가 없어야 한다는 테스트 3건이 실패하는 것을 확인한 뒤 구현.
- 실샘플 4종 재확인 (실물 `.hwp` 의 FileHeader 플래그 비트를 직접 뒤집어 만들었다 — 합성 헤더가 아니라 진짜 CFB 문서다):
  - 원본 → 성공 2.0s
  - 배포용(bit2) → 성공 + "배포용(읽기 전용) 한글 문서입니다…" 안내
  - 암호(bit1) → "암호가 설정된 한글 문서입니다. 한글에서 암호를 해제한 뒤…"
  - 1/3 만 남긴 잘린 파일 · 확장자만 `.hwp` 인 텍스트 → "문서를 열지 못했습니다. 한글 문서가 아니거나 파일이 손상됐습니다."
- `cargo test` 264 · Vitest 61 그린, clippy·fmt·tsc·eslint·prettier 통과.

## 메모

실제로 암호화된 문서(한글에서 암호를 걸어 저장한 파일)는 아직 못 구했다. 앱은 FileHeader 플래그만 보고 변환 전에 막으므로 동작은 같지만, 진짜 파일로 한 번은 확인해 둘 자리다.