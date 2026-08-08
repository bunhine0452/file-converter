---
schema_version: 1
type: feature
slug: "settings-screen-and-native-titlebar"
status: done
difficulty: medium
created_at: "2026-08-08T21:58:35+09:00"
session_id: "mcp-20260808-215835"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "src-tauri/src/core/settings.rs"
    op: create
  - path: "src-tauri/src/core/output.rs"
    op: update
  - path: "src-tauri/src/shell/commands.rs"
    op: update
  - path: "src-tauri/src/shell/mod.rs"
    op: update
  - path: "src-tauri/src/lib.rs"
    op: update
  - path: "src-tauri/tauri.conf.json"
    op: update
  - path: "src/lib/settings.ts"
    op: create
  - path: "src/lib/theme.ts"
    op: create
  - path: "src/lib/platform.ts"
    op: create
  - path: "src/hooks/useSettings.ts"
    op: create
  - path: "src/components/SettingsPanel.tsx"
    op: create
  - path: "src/App.tsx"
    op: update
  - path: "src/main.tsx"
    op: update
  - path: "src/index.css"
    op: update
related: []
tags:
  - "phase3"
  - "설정"
  - "테마"
  - "tauri"
  - "ux"
  - "tdd"
  - "mcp-tool"
---
[x] 설정 화면과 네이티브 타이틀바를 붙여 앱 셸을 마무리했다

Phase 3 의 남은 두 항목(`shell-settings`, `shell-window`)을 함께 처리했다.

## 추가 기능

**설정 화면**

- **저장 위치** — 변환할 때마다 묻기(기본) / 원본과 같은 폴더 / 지정한 폴더. 정해 둔 방식이면 대화상자를 아예 띄우지 않는다. 저장 위치를 정해 둔 사용자에게 매번 묻는 것은 설정을 무시하는 것이다.
- **파일 이름** — 접미사(비우면 원본 이름 그대로)와 **결과 이름 미리보기**(`보고서.hwp → 보고서_변환.pdf`). 규칙을 글로만 설명하면 무슨 파일이 나올지 알 수 없다.
- **같은 이름이 있을 때** — 번호 붙이기 / 덮어쓰기. `plan_output_path` 가 이 설정을 따른다.
- **테마** — 시스템 / 라이트 / 다크.
- **변환 런타임** — 기존 RuntimeStatus 를 설정 화면으로 옮겼다(사이드바와 동시에 띄우지 않아 조회도 한 번만 돈다).

**네이티브 타이틀바**

macOS 는 `titleBarStyle: "Overlay"` + `hiddenTitle` 로 타이틀바를 투명하게 깔고 신호등 버튼만 남긴다. 사이드바 상단에 그 자리를 비우고(1.75rem) 제목 영역에 `data-tauri-drag-region` 을 줘 창을 옮길 수 있게 했다. Windows/Linux 는 시스템 타이틀바를 그대로 쓰므로 여백을 주지 않는다 — 판별은 UA 의 `Macintosh` 로 한다(iPhone UA 에도 "Mac OS X" 가 들어가 오인하기 쉽다).

## 동작 흐름

설정은 앱 데이터 루트의 `settings.json` 에 둔다 — 런타임 디렉토리를 통째로 지워도 설정은 남아야 한다. **읽기는 절대 실패하지 않는다**: 파일이 없거나 깨졌으면 기본값으로 시작하고 다음 저장에서 정상 파일이 된다. 설정 하나 때문에 앱이 안 뜨면 사용자는 복구할 방법이 없다.

테마는 CSS 에 `[data-theme="dark"]` 한 벌만 두고 "시스템"을 실제 밝기로 바꾸는 일은 `lib/theme.ts` 가 한다. 미디어 쿼리와 명시 선택을 둘 다 CSS 에 두면 다크 토큰을 두 벌 적어야 하고, 그 순간부터 둘이 어긋난다. 첫 프레임에 흰 화면이 번쩍이지 않도록 `main.tsx` 가 렌더 전에 OS 테마를 먼저 적용한다.

"지정한 폴더"인데 폴더를 아직 안 골랐으면 묻는 쪽으로 되돌린다. 반쯤 빈 설정으로 말없이 아무 데나 저장하면 사용자가 파일을 잃는다.

## 검증

- `cargo test` 271 → 283, Vitest 91 → 114 그린. 새 동작은 전부 RED 확인 후 구현.
- 설정 파싱은 깨진 입력 5종(`""`, `"{"`, `null`, `[]`, 텍스트)과 부분 필드·모르는 필드·모르는 값까지 테스트.
- clippy 0 · fmt · tsc · eslint · prettier · `vite build` 통과.
- `pnpm tauri dev` 로 앱을 띄워 기동 확인(런타임 에러 없음).

## 메모

화면을 눈으로 보지는 못했다 — 이 환경에 화면 기록 권한이 없어 `screencapture` 가 창을 캡처하지 못한다(`could not create image from window`). 토큰 대비는 테스트로 고정했고 레이아웃은 빌드까지 통과했지만, **신호등 버튼이 실제로 겹치지 않는지·다크 테마가 자연스러운지는 사람 눈이 필요하다.** 앱은 띄워 둔 채로 뒀다.

설정 파싱에서 모르는 enum 값을 만나면(예: 신버전이 쓴 `saveMode`) serde 가 전체 파싱에 실패해 **설정 전부가 기본값으로 되돌아간다.** 지금은 테스트로 그 동작을 못박아 뒀지만, 필드별 폴백이 필요해지면 그때 갈라야 한다.