# 라이선스 경계 (앱 본체 MIT)

앱 본체는 MIT로 배포한다. GPL/LGPL 도구는 **프로세스 경계** 너머에서만 사용한다.

- H2Orestart(GPLv3)·LibreOffice(MPL-2.0): 코드 링크 금지 — `soffice` 외부 프로세스 호출만 허용.
- FFmpeg: LGPL 빌드만 사이드카로 번들 (GPL 코덱 포함 빌드 금지).
- GPL 라이브러리를 Cargo.toml/package.json 의존성으로 추가하지 않는다 (외부 바이너리 호출로 대체).
- 번들·다운로드하는 서드파티 도구는 THIRD-PARTY-NOTICES에 라이선스 고지를 추가한 뒤 머지한다.
