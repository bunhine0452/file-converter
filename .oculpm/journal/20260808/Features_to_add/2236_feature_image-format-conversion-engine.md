---
schema_version: 1
type: feature
slug: "image-format-conversion-engine"
status: done
difficulty: low
created_at: "2026-08-08T22:36:45+09:00"
session_id: "mcp-20260808-223645"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "src-tauri/src/core/image/convert.rs"
    op: create
  - path: "src-tauri/src/core/image/mod.rs"
    op: create
  - path: "src-tauri/src/core/mod.rs"
    op: update
  - path: "src-tauri/Cargo.toml"
    op: update
related: []
tags:
  - "phase4"
  - "이미지"
  - "rust"
  - "tdd"
  - "mcp-tool"
---
[x] 이미지 6종 상호 변환 엔진을 매트릭스로 검증하며 세웠다

Phase 4 의 첫 조각 — `img-basic`.

## 추가 기능

PNG · JPG · WebP · BMP · TIFF · GIF 6종 상호 변환. 바이트를 받아 바이트를 돌려주는 순수 함수라 실제 파일 없이 **6×6 = 36 조합**을 한 테스트에서 검증한다(견본을 그때그때 인코딩해 만든다).

`image` 0.25 를 기본 기능을 끄고 필요한 코덱만 켜서 넣었다. 새로 들어온 의존성을 포함해 533개 패키지의 라이선스를 훑어 GPL 0건을 확인했다(앱 본체 MIT 경계).

## 동작 흐름

투명도를 담을 수 없는 포맷(JPG·BMP)으로 갈 때는 **흰 배경에 합성**한다. 알파를 그냥 버리면 투명했던 자리가 새까맣게 나온다 — "사진을 JPG 로 바꿨더니 배경이 검더라"는 전형적인 사고라 완전 투명 PNG → JPG 결과가 흰색인지 테스트로 못박았다.

깨진 입력은 라이브러리 원문 대신 한국어 안내로 거절한다. 오늘 HWP 쪽에서 `Invalid CFB file...` 이 그대로 새어 나간 것을 고쳤는데, 같은 실수를 이미지에서 되풀이하지 않으려고 처음부터 메시지 계층을 갈라 뒀다.

## 검증

- `cargo test` 296 → 301 그린. 매트릭스·알파 합성·깨진 입력·동일 포맷 변환 전부 RED 확인 후 구현.
- clippy 0 · fmt 통과.

## 메모

아직 엔진뿐이다 — 품질·리사이즈·메타데이터 제거(`img-options`), HEIC(`img-heic`), 일괄 UX(`img-batch`)는 다음 조각이다. UI 의 "이미지" 분류도 여전히 "준비 중"으로 잠겨 있다.