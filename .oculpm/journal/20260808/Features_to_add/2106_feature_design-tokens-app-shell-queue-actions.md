---
schema_version: 1
type: feature
slug: "design-tokens-app-shell-queue-actions"
status: done
difficulty: medium
created_at: "2026-08-08T21:06:50+09:00"
session_id: "mcp-20260808-210650"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "src/index.css"
    op: update
  - path: "src/App.tsx"
    op: update
  - path: "src/components/CategoryNav.tsx"
    op: create
  - path: "src/components/CategoryNav.test.tsx"
    op: create
  - path: "src/components/ConversionQueue.tsx"
    op: update
  - path: "src/components/ConversionQueue.test.tsx"
    op: update
  - path: "src/hooks/useConversionQueue.ts"
    op: update
  - path: "src/hooks/useConversionQueue.test.ts"
    op: update
  - path: "src/lib/color.ts"
    op: create
  - path: "src/lib/color.test.ts"
    op: create
  - path: "tsconfig.json"
    op: update
related: []
tags:
  - "phase3"
  - "디자인시스템"
  - "tailwind"
  - "접근성"
  - "ux"
  - "tdd"
  - "mcp-tool"
---
[x] 디자인 토큰과 앱 셸을 세우고 큐에 실제 조작을 붙였다

Phase 3 첫 슬라이스. "여기서 더 UX를 향상시켜 달라"는 요청에 따라 토큰·셸·조작을 함께 세웠다.

## 추가 기능

**디자인 토큰 (#tokens-define, #tokens-typography, #tokens-shadcn)**

방향은 *차분한 유틸리티* — 매일 쓰는 로컬 도구다. 화면의 주인공은 장식이 아니라 사용자의 파일이므로 중성색은 아주 낮은 채도(hue 250)로 깔고, **색은 상태에만** 쓴다.

- 상태색 신설: `accent-strong`(진행) · `success`(완료) · `destructive`(실패). 완료가 회색이면 목록에서 결과를 훑을 수 없다.
- 깊이는 2단계(`raised`/`overlay`)만. 그림자를 남발하면 목록이 울퉁불퉁해진다.
- 모션 토큰(`--motion-fast/normal`, `--ease-out-soft`)과 전역 `prefers-reduced-motion` 차단(#motion-polish).
- 한글 서체는 **번들하지 않고** OS 기본(Apple SD Gothic Neo / 맑은 고딕)을 우선한다 — 설치 용량과 오프라인 원칙을 지키면서 각 OS 에서 가장 자연스럽다. 자간 -0.01em, 숫자는 tabular.
- 다크는 OS 설정을 따른다(`prefers-color-scheme`). 토글은 설정 화면(#shell-settings)에서 붙인다.

**앱 셸 (#shell-nav)**

사이드바(문서/이미지/PDF/미디어) + 메인 큐. 아직 못 만든 분류는 "준비 중"으로 비활성화하고 탭 순서에서도 뺐다 — 눌리는데 아무 일도 일어나지 않는 메뉴가 제일 나쁘다. 접근성 이름은 분류명만 두고 설명은 `aria-describedby` 로 뺐다(그러지 않으면 "PDF → 이미지 추출"과 "이미지"가 스크린리더에서 구분되지 않는다).

**큐 조작**

- **변환 취소** — `cancel_job` 커맨드는 진작 있었는데 화면에 붙어 있지 않아 아무도 쓸 수 없었다.
- **PDF 바로 열기** + 저장 위치 열기. 저장 위치만 열어 주면 사용자가 파일을 또 찾아야 한다.
- 진행 막대(`role="progressbar"`) — 대용량 변환에서 숫자만으로는 살아있는지 알기 어렵다.
- 끝난 항목 지우기 — 진행 중인 항목은 남긴다.

## 동작 흐름

`useConversionQueue` 가 `clearFinished` 를 내주고, 종료 상태(완료·실패·취소됨)만 걸러낸다. "취소 중"은 아직 끝난 게 아니라 정리 대상이 아니지만 진행률은 받지 않는다 — 두 개념을 상수 두 개(`FINISHED_STATUSES` / `SETTLED_STATUSES`)로 갈랐다.

## 검증

- Vitest 61 → 89 그린. 새 동작(취소·열기·막대·정리·내비게이션)은 전부 RED 확인 후 구현.
- **대비 테스트(#a11y-contrast)**: `index.css` 의 토큰 값을 파일에서 직접 읽어 라이트/다크 10쌍이 AA(4.5:1)를 넘는지 검사한다. 최저 5.59:1. 값을 테스트에 복사해 두면 CSS 만 고쳤을 때 옛 색을 검사하게 되므로 파일을 읽게 했고, 토큰을 나쁜 값으로 바꿔 **실제로 실패하는 것까지** 확인했다.
- `tsc --noEmit`·eslint·prettier·`vite build` 통과 (CSS 24.6KB → gzip 5.3KB).

## 메모

앱 레이아웃을 바꾸면서 기존 App 테스트 2건이 깨졌는데, 기대를 낮추는 대신 마크업을 고쳤다 — 제품명을 `h1` 으로 올리고(창의 최상위 제목은 앱 이름) 접근성 이름을 목록(`ul`) 자체에 붙였다. 테스트가 옳았고 내 마크업이 틀렸다.

남은 것: 설정 화면(#shell-settings), 네이티브 타이틀바(#shell-window), 그리고 실제 앱 창에서 키보드만으로 끝까지 가 보는 확인(#a11y-keyboard).