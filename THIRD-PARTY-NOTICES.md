# Third-Party Notices (초안)

이 앱은 아래 서드파티 도구를 **외부 프로세스/사이드카**로 사용합니다. 앱 본체(MIT)와 코드가 링크되지 않습니다.

| 구성요소 | 라이선스 | 사용 방식 |
|---|---|---|
| [LibreOffice](https://www.libreoffice.org/) | MPL-2.0 | 온디맨드 다운로드, `soffice --headless` 외부 프로세스 (HWP/사무 문서 변환) |
| [H2Orestart](https://github.com/ebandal/H2Orestart) | GPL-3.0 | LibreOffice 확장으로 설치되어 LibreOffice 내부에서 실행 (HWP/HWPX 임포트 필터) |
| [Eclipse Temurin JRE 21](https://adoptium.net/) | GPL-2.0 with Classpath Exception | 온디맨드 다운로드. H2Orestart 가 Java 확장이라 LibreOffice 가 실행할 JRE 가 필요하다 |
| [FFmpeg](https://ffmpeg.org/) | LGPL-2.1+ (LGPL 빌드만 번들) | 사이드카 바이너리 (미디어 변환) |
| [PDFium](https://pdfium.googlesource.com/pdfium/) | Apache-2.0/BSD | pdfium-render 크레이트 경유 (PDF 렌더·유틸) |

빌드에 포함되는 Rust/npm 의존성 고지는 릴리스 파이프라인에서 자동 생성해 이 문서에 병합 예정.
