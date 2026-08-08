---
schema_version: 1
type: bug
slug: "hwp-korean-font-substitution"
status: done
difficulty: high
created_at: "2026-08-08T22:32:15+09:00"
session_id: "mcp-20260808-223215"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "src-tauri/src/core/fonts.rs"
    op: create
  - path: "src-tauri/src/core/runtime/assets.rs"
    op: update
  - path: "src-tauri/src/core/runtime/installer.rs"
    op: update
  - path: "src-tauri/src/core/runtime/plan.rs"
    op: update
  - path: "src-tauri/src/shell/runtime_manager.rs"
    op: update
  - path: "src-tauri/src/shell/commands.rs"
    op: update
  - path: "src/lib/runtime.ts"
    op: update
  - path: "src/components/RuntimeStatus.tsx"
    op: update
  - path: "THIRD-PARTY-NOTICES.md"
    op: update
related: []
tags:
  - "phase2"
  - "hwp"
  - "글꼴"
  - "libreoffice"
  - "실환경"
  - "품질"
  - "tdd"
  - "mcp-tool"
---
[x] 한글이 사라지고 서식이 틀어진 원인은 글꼴이었다

사용자 보고: "PDF 로 변환하니 깨지는 한글도 있고 서식·표도 제대로 안 된다."

## 발생 원인

추측하지 않고 산출물을 봤다. PDFKit 으로 페이지를 렌더해 놓고 보니 `｢중소기업기본법｣` 의 **낫표가 통째로 비어** 있었다. PDF 텍스트 층에는 문자가 멀쩡히 있었다(`U+FF62/FF63`, 반각 낫표) — 즉 **글리프가 없어서 안 그려진** 것이다.

PDF 가 품고 있는 글꼴을 뽑아 보니 원인이 드러났다.

```
GabiaSai-Regular, STHeitiSC-Medium, AppleMyungjo,
AppleSDGothicNeo, LiberationSerif, Helvetica ...
```

문서(header.xml)가 요구한 글꼴은 함초롬바탕·함초롬돋움·한컴바탕·HY견고딕·휴먼명조·한양신명조 등 **한컴 계열 20종**인데, 이 기계에는 하나도 없다. 그래서 LibreOffice 가 제멋대로 골랐고, 하필 사용자 PC 에 깔려 있던 **장식용 손글씨체(GabiaSai)** 와 **중국어 폰트(STHeitiSC)** 까지 본문에 끌려왔다. 한 문서 안에서 대체 글꼴이 뒤섞이니 자간·줄바꿈·표 폭이 전부 틀어진다 — 이것이 "서식·표가 깨진다"의 정체였다.

낫표는 더 나빴다. CoreText 로 확인해 보니 **맥에 있는 한글 글꼴 어디에도** `U+FF62/63` 이 없었다(Apple SD Gothic Neo·AppleMyungjo·STHeiti 모두 없음). LibreOffice 가 번들한 135개 글꼴에도 없었다. 즉 글꼴을 새로 들여오지 않는 한 이 글자는 영영 안 나온다.

## 해결 방법

1. **글꼴을 갖춰 준다** — Noto Sans KR / Noto Serif KR(SIL OFL, 4.6MB+7.7MB)을 해시 검증 후 내려받아 **앱이 설치한** LibreOffice 의 글꼴 폴더에 넣는다. 두 글꼴 모두 낫표·원문자·㎥ 까지 커버하는 것을 cmap 파싱으로 확인하고 채택했다. 사용자의 시스템 글꼴 폴더나 사용자가 직접 설치한 LibreOffice 는 건드리지 않는다.
2. **대체를 우연에 맡기지 않는다** — 한컴 계열 26개 이름을 명조↔세리프, 고딕↔산세리프로 갈라 Noto 로 잇는 규칙을 넣는다. 이름을 아는 것만 규칙으로 뒀다. 모르는 글꼴까지 싸잡아 바꾸면 사용자가 실제로 가진 글꼴을 빼앗는다.
3. **규칙을 어디에 적을 것인가** — `share/registry/*.xcu` 에 넣어 봤지만 **무시당했다**(대조 실험: 규칙 파일이 있을 때와 없을 때 산출물 글꼴이 동일). LibreOffice 가 확실히 읽는 곳은 전용 프로필의 `registrymodifications.xcu` 였다. 대체표는 기본값이 꺼져 있어 `Replacement=true` 도 함께 켜야 한다.
4. **자가 치유** — 프로필을 지우면 규칙도 사라지므로 변환 직전에 없으면 다시 채운다.
5. **정직한 상태** — 글꼴이 없으면 `needsFonts` 로 드러낸다. 변환은 되지만 글자가 깨지는 상태를 "준비됨"이라 부르지 않는다.

## 검증

- 같은 문서 재변환 시 PDF 가 품는 글꼴: `GabiaSai·STHeitiSC·AppleMyungjo·LiberationSerif…` → **`NotoSansKR·NotoSerifKR`**(+굵은 제목용 AppleSDGothicNeo). 사라졌던 `｢｣` 가 정상 출력되고, 라틴 세리프로 떨어져 두부(□)가 되던 구절도 사라졌다.
- 실제 설치 경로로 재현: 글꼴·프로필을 지우고 `install` → 다운로드·배치·규칙 적용 확인. 프로필만 지우고 `convert` → 자가 치유 확인.
- 표 위주 문서(.hwp)도 단일 글꼴로 일관되게 렌더된다.
- `cargo test` 283 → 296 그린. 새 로직(대체 규칙 XML·병합 멱등성·글꼴 디렉토리·상태 라벨)은 전부 RED 확인 후 구현. clippy 0 · fmt · tsc · eslint · prettier 통과.

## 메모

대체 글꼴의 자폭·행간은 함초롬과 다르므로 **줄바꿈 위치와 페이지 나눔은 원본과 완전히 같을 수 없다.** 실제로 텍스트 상자 하단이 한 줄 잘리는 경우를 봤다. 원본과 픽셀 단위로 같게 하려면 함초롬체 자체가 필요한데 재배포 라이선스가 불분명해 넣지 않았다 — 사용자가 한컴 오피스를 설치해 두었다면 그 글꼴이 우선 사용된다.

조사 도구로 만든 것(재사용 가치 있음): PDFKit 기반 페이지 렌더러/텍스트 추출기, CoreText 글리프 커버리지 검사기, cmap 파서. 모두 `/tmp` 에 두었고 커밋하지는 않았다.