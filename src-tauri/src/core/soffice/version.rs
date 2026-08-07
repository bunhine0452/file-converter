//! LibreOffice 버전 — `soffice --version` 출력 파싱과 동작 계약 판정.

/// 이 버전 이상에서만 실패 시 exit code 를 신뢰할 수 있다 (tdf#148275).
pub const EXIT_CODE_TRUSTWORTHY_FROM: (u32, u32) = (26, 2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoVersion {
    pub major: u32,
    pub minor: u32,
    pub micro: u32,
    pub patch: u32,
}

impl LoVersion {
    pub fn new(major: u32, minor: u32, micro: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            micro,
            patch,
        }
    }

    /// 26.2 미만은 변환에 실패해도 exit 0 을 내므로 종료 코드를 근거로 쓸 수 없다.
    pub fn trusts_exit_code(&self) -> bool {
        (self.major, self.minor) >= EXIT_CODE_TRUSTWORTHY_FROM
    }
}

impl std::fmt::Display for LoVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.major, self.minor, self.micro, self.patch
        )
    }
}

/// `soffice --version` 의 첫 줄에서 버전을 뽑는다.
///
/// 출력 형식은 `"%PRODUCTNAME %PRODUCTVERSION%PRODUCTEXTENSION %BUILDID"` 이므로
/// 제품명(첫 토큰)은 `LibreOfficeDev` 등으로 달라질 수 있다 — 두 번째 토큰만 믿는다.
pub fn parse_version(stdout: &str) -> Option<LoVersion> {
    let first_line = stdout.lines().find(|line| !line.trim().is_empty())?;
    let version_token = first_line.split_whitespace().nth(1)?;

    let mut segments = version_token.split('.');
    let major = parse_segment(segments.next())?;
    let minor = parse_segment(segments.next())?;
    let micro = parse_segment(segments.next()).unwrap_or(0);
    let patch = parse_segment(segments.next()).unwrap_or(0);

    Some(LoVersion::new(major, minor, micro, patch))
}

fn parse_segment(segment: Option<&str>) -> Option<u32> {
    // "26.2.5.2~rc1" 처럼 꼬리가 붙는 배포본이 있어 앞쪽 숫자만 취한다.
    let segment = segment?;
    let digits: String = segment.chars().take_while(|c| c.is_ascii_digit()).collect();

    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 네_자리_버전을_그대로_읽는다() {
        let version = parse_version("LibreOffice 26.2.5.2 f1a2b3c\n\n").expect("파싱 성공");

        assert_eq!(version, LoVersion::new(26, 2, 5, 2));
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 제품명이_달라도_두번째_토큰만_본다() {
        let version = parse_version("LibreOfficeDev 26.2.5.2 x").expect("파싱 성공");

        assert_eq!(version, LoVersion::new(26, 2, 5, 2));
    }

    #[test]
    fn 세_자리_버전은_패치를_0_으로_채운다() {
        let version = parse_version("LibreOffice 25.8.6 x\n").expect("파싱 성공");

        assert_eq!(version, LoVersion::new(25, 8, 6, 0));
    }

    #[test]
    fn 배포본_꼬리표가_붙어도_숫자만_취한다() {
        let version = parse_version("LibreOffice 26.2.5.2~rc1 x").expect("파싱 성공");

        assert_eq!(version, LoVersion::new(26, 2, 5, 2));
    }

    #[test]
    fn 앞의_빈_줄은_건너뛴다() {
        let version = parse_version("\n\nLibreOffice 26.2.5.2 x").expect("파싱 성공");

        assert_eq!(version, LoVersion::new(26, 2, 5, 2));
    }

    #[test]
    fn 두번째_토큰이_숫자가_아니면_none() {
        assert!(parse_version("LibreOffice unknown\n").is_none());
    }

    #[test]
    fn 토큰이_하나뿐이면_none() {
        assert!(parse_version("LibreOffice\n").is_none());
    }

    #[test]
    fn 빈_출력은_none() {
        assert!(parse_version("").is_none());
        assert!(parse_version("   \n\n").is_none());
    }

    #[test]
    fn exit_code_는_26_2_부터만_신뢰한다() {
        assert!(!LoVersion::new(25, 8, 6, 0).trusts_exit_code());
        assert!(!LoVersion::new(26, 1, 9, 9).trusts_exit_code());
        assert!(LoVersion::new(26, 2, 0, 0).trusts_exit_code());
        assert!(LoVersion::new(27, 0, 0, 0).trusts_exit_code());
    }

    #[test]
    fn 버전은_사전순이_아니라_수치로_비교된다() {
        assert!(LoVersion::new(26, 2, 5, 2) > LoVersion::new(26, 2, 5, 1));
        assert!(LoVersion::new(26, 10, 0, 0) > LoVersion::new(26, 9, 0, 0));
    }
}
