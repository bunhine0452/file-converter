---
schema_version: 1
type: chore
slug: "project-inception-file-converter"
status: done
difficulty: medium
created_at: "2026-08-07T14:56:38+09:00"
session_id: "mcp-20260807-145638"
agent:
  id: "claude-code"
  version: "Fable 5"
language: "ko"
verified_by_user: false
files_touched:
  - path: ".oculpm/discussion/file-converter-stack/discussion.md"
    op: create
  - path: ".oculpm/planner/file-converter-v1.md"
    op: create
  - path: "EVALS.md"
    op: create
  - path: ".claude/rules/stack-versions.md"
    op: create
  - path: ".claude/rules/local-only-conversion.md"
    op: create
  - path: ".claude/rules/license-boundaries.md"
    op: create
  - path: "README.md"
    op: create
  - path: "LICENSE"
    op: create
  - path: "THIRD-PARTY-NOTICES.md"
    op: create
related: []
tags:
  - "inception"
  - "planning"
  - "research"
  - "mcp-tool"
---
[x] 프로젝트 인셉션 — 스택 확정·계획·EVALS·규칙·퍼블릭 리포 생성

## 추가 기능

설치형 파일 변환기(HWP/HWPX→PDF + 사무 문서·이미지·PDF·미디어) 인셉션 완료. 웹 리서치로 스택을 확정하고 4종 산출물(discussion·plan·EVALS·rules)을 생성, GitHub 퍼블릭 리포를 개설했다.

## 동작 흐름

- 리서치: HWP 로컬 변환의 유일 현실 경로 = LibreOffice 26.2.5 headless + H2Orestart 0.7.13(GPLv3, 외부 프로세스). 셸은 Tauri 2.11.5 vs Electron 42.8.1 비교 후 사용자 선택.
- 확정: Tauri 2 + React 19 + Tailwind v4 + shadcn/ui 커스텀, LibreOffice 감지+온디맨드, MIT 라이선스.
- 계획: file-converter-v1 (7 phases, 71 items). 세션 중 사용자 추가 요청으로 Phase 5를 사무 문서 전방위(DOC/ODT/RTF/CSV, HWP→DOCX 등)로 확장.
- 저장소: https://github.com/bunhine0452/file-converter 퍼블릭 생성, 초기 커밋 푸시 (LICENSE·README·THIRD-PARTY-NOTICES 포함).

## 검증

- `gh repo create --push` 성공, main 브랜치 추적 확인.
- plan_create 응답 7 phases/71 items, discussion status=resolved.