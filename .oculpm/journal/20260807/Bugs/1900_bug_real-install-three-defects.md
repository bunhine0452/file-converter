---
schema_version: 1
type: bug
slug: "real-install-three-defects"
status: done
difficulty: superhigh
created_at: "2026-08-07T19:00:43+09:00"
session_id: "mcp-20260807-190043"
agent:
  id: "claude-code"
  version: "Opus 5 (1M context)"
language: "ko"
verified_by_user: false
files_touched:
  - path: "src-tauri/examples/verify_runtime.rs"
    op: create
  - path: "src-tauri/src/core/runtime/installer.rs"
    op: update
  - path: "src-tauri/src/core/runtime/real_installer.rs"
    op: update
  - path: "src-tauri/src/core/runtime/assets.rs"
    op: update
  - path: "src-tauri/src/core/runtime/plan.rs"
    op: update
  - path: "src-tauri/src/shell/runtime_manager.rs"
    op: update
related: []
tags:
  - "phase2"
  - "libreoffice"
  - "h2orestart"
  - "macos"
  - "실환경검증"
  - "mcp-tool"
---
[x] 실제 설치 실행으로 드러난 결함 3건과 남은 블로커

## 발생 원인

앱을 띄우지 않고 런타임 설치를 그대로 돌리는 하네스(`examples/verify_runtime`)를 만들어 실제로 실행했다. 단위 테스트가 모두 그린인 상태에서 세 가지가 나왔다.

1. **빈 디렉토리를 `JAVA_HOME` 으로 인정** — 설치기(`install_jre`)는 `bin/java` 존재를 확인했는데 `RuntimeManager::find_java_home` 은 디렉토리만 봤다. 다운로드가 끊겨 `jre/` 폴더만 남은 상태에서 상태가 "Java 준비됨"으로 나왔고, 그러면 설치 계획이 JRE 단계를 영영 건너뛴다. 같은 판정을 두 곳에 따로 쓴 것이 원인.
2. **확장자를 잃어 `.tar.gz` 를 zip 으로 풀었다** — 내려받은 파일을 `downloads/jre` 로 저장했다. 해시 검증은 통과한 뒤라 `Could not find EOCD` 만 보고는 원인이 안 보였다.
3. **남은 `.lock` 이 이후 모든 호출을 막았다** — 비정상 종료로 프로필에 잠금이 남으면 unopkg 가 시작조차 못 한다.

덤으로 `InstallExtension` 이 계획의 전략(BundledDir/UserProfile)을 무시하고 항상 unopkg 를 쓰고 있었다.

## 해결 방법

- 판정을 `resolve_java_home` 하나로 합쳐 설치기와 매니저가 같은 규칙을 쓰게 했다 (`bin/java` 존재 필수).
- 내려받은 파일은 URL 의 파일명을 그대로 쓴다 (`asset_file_name`). 모든 플랫폼 자산에 확장자가 있는지 테스트로 잠갔다.
- 잠금 메시지를 감지하는 순수 함수(`is_stale_lock_error`)를 두고, 프로필을 쓰는 모든 실행을 "감지되면 한 번 치우고 재시도"로 감쌌다. 전용 프로필이라 이 잠금은 항상 우리가 남긴 찌꺼기다.
- 전략에 따라 번들 디렉토리 배치 / unopkg 를 갈라 쓰게 했다.

각 결함에 재발 방지 테스트를 붙였다 (cargo test 218→225).

## 검증

- `cargo test` 225 / Vitest 50 그린, clippy 경고 0, CI macOS·Windows 그린 (PR #5, main 머지)
- 실환경에서 **확인된 것**: macOS 번들 탐지와 `soffice --version` 파싱이 실물 LibreOffice 26.2.5.2 에서 동작 / dmg 297MB 다운로드·해시 검증·hdiutil+ditto 설치 완료 / JRE tar.gz 48MB 설치 완료, `java -version` 정상 / LibreOffice 가 우리 JRE(Temurin 21.0.12)를 자동 탐지해 프로필에 기록 — `JAVA_HOME` 주입만으로 **탐지는 된다**

## 메모

**남은 블로커**: macOS 에서 `unopkg add` 가 H2Orestart 의 Java 컴포넌트를 활성화하지 못한다.

```
An error occurred while enabling: H2Orestart.jar:
com.sun.star.connection.NoConnectException "Connector : couldn't connect to pipe ...": 10
```

배제한 원인 — JRE 실행 불가(아님, `java -version` 정상) / Gatekeeper 격리(아님, xattr 에 quarantine 없음) / `javasettings` 의 `enabled` 플래그(`true` 로 바꿔도 동일) / 잠금 파일(제거 후에도 동일) / 프로필 URL 인코딩(정상).

비-Java 하위 패키지는 `is registered: yes` 가 되고 `.jar` 만 실패한다. 번들 디렉토리에 풀어둔 것은 `unopkg list --bundled` 에 아예 잡히지 않는다 — 단순 배치만으로는 등록되지 않는 듯하다.

다음에 시도할 것: `soffice` 를 먼저 한 번 완전히 기동해 UNO 파이프를 살려둔 상태에서 unopkg 를 붙이기 / `--shared` 없이 `unopkg add -v` 로 상세 로그 보기 / LibreOffice 를 `/Applications` 에 정식 설치했을 때도 재현되는지 대조 / H2Orestart 이슈 트래커에서 macOS aarch64 사례 확인.