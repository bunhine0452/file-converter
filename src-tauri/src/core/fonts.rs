//! 한글 글꼴 대체 규칙.
//!
//! HWP 문서는 함초롬바탕·한컴바탕·HY견고딕 같은 한컴 계열 글꼴을 쓰는데, 이 글꼴들은
//! 한컴 오피스를 깔지 않은 기계에는 없다. LibreOffice 는 이름을 모르면 아무거나 고르는데
//! 실측해 보니 장식용 손글씨체(GabiaSai)나 중국어 폰트(STHeiti)까지 끌려왔고, 심지어
//! 라틴 세리프(Liberation Serif)를 골라 한글이 통째로 두부(□)가 되기도 했다.
//!
//! 그래서 우리가 설치한 Noto 한글 글꼴로 **명시적으로** 이어 준다. 이름을 아는 것만
//! 규칙으로 둔다 — 모르는 글꼴까지 싸잡아 바꾸면 사용자가 실제로 가진 글꼴을 빼앗는다.

/// 앱이 설치하는 한글 본문 글꼴 (SIL OFL).
pub const KOREAN_SERIF: &str = "Noto Serif KR";
pub const KOREAN_SANS: &str = "Noto Sans KR";

/// 명조/바탕 계열 — 세리프로 잇는다.
const SERIF_FAMILIES: &[&str] = &[
    "함초롬바탕",
    "한컴바탕",
    "한컴바탕확장",
    "한양신명조",
    "휴먼명조",
    "신명조",
    "HY신명조",
    "HY중고딕",
    "명조",
];

/// 고딕/돋움 계열 — 산세리프로 잇는다.
const SANS_FAMILIES: &[&str] = &[
    "함초롬돋움",
    "한컴돋움",
    "한컴산뜻돋움",
    "한컴 윤고딕 230",
    "한컴 윤고딕 240",
    "한컴 윤체 B",
    "HY견고딕",
    "HY헤드라인M",
    "HY울릉도B",
    "HY울릉도M",
    "HY얕은샘물M",
    "맑은 고딕",
    "MalgunGothic",
    "KoPub돋움체 Light",
    "KoPub돋움체 Medium",
    "KoPub돋움체 Bold",
    "HCI Poppy",
];

/// LibreOffice 프로필 레지스트리의 대체표 경로.
const FONT_PAIRS_PATH: &str = "/org.openoffice.Office.Common/Font/Substitution/FontPairs";
const SUBSTITUTION_PATH: &str = "/org.openoffice.Office.Common/Font/Substitution";
const REGISTRY_END: &str = "</oor:items>";

/// 대체 규칙 수 (설치 검증에 쓴다).
pub fn substitution_count() -> usize {
    SERIF_FAMILIES.len() + SANS_FAMILIES.len()
}

/// 프로필 레지스트리에 넣을 `<item>` 들.
pub fn substitution_items() -> String {
    // 대체표는 기본값이 꺼져 있다 — 규칙만 넣고 켜지 않으면 아무 일도 일어나지 않는다.
    let mut xml = format!(
        "<item oor:path=\"{SUBSTITUTION_PATH}\">\
         <prop oor:name=\"Replacement\" oor:op=\"fuse\"><value>true</value></prop></item>"
    );

    for (families, substitute) in [(SERIF_FAMILIES, KOREAN_SERIF), (SANS_FAMILIES, KOREAN_SANS)] {
        for family in families {
            xml.push_str(&font_pair(family, substitute));
        }
    }

    xml
}

fn font_pair(family: &str, substitute: &str) -> String {
    format!(
        "<item oor:path=\"{FONT_PAIRS_PATH}\">\
         <node oor:name=\"{family}\" oor:op=\"replace\">\
         <prop oor:name=\"ReplaceFont\" oor:op=\"fuse\"><value>{family}</value></prop>\
         <prop oor:name=\"SubstituteFont\" oor:op=\"fuse\"><value>{substitute}</value></prop>\
         <prop oor:name=\"Always\" oor:op=\"fuse\"><value>true</value></prop>\
         <prop oor:name=\"OnScreenOnly\" oor:op=\"fuse\"><value>false</value></prop>\
         </node></item>"
    )
}

/// 이미 규칙이 들어 있는 레지스트리인가.
pub fn has_substitutions(registry_xml: &str) -> bool {
    registry_xml.contains(FONT_PAIRS_PATH)
}

/// 레지스트리 XML 끝에 규칙을 끼워 넣은 새 문서.
///
/// 이미 있거나(중복 금지) 형태를 알아볼 수 없으면 `None` — 남의 설정 파일을 망가뜨리는
/// 것보다 글꼴 대체를 포기하는 편이 낫다.
pub fn merge_substitutions(registry_xml: &str) -> Option<String> {
    if has_substitutions(registry_xml) {
        return None;
    }

    let end = registry_xml.rfind(REGISTRY_END)?;

    Some(format!(
        "{}{}\n{}",
        &registry_xml[..end],
        substitution_items(),
        &registry_xml[end..]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_registry() -> String {
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <oor:items xmlns:oor=\"http://openoffice.org/2001/registry\">\n\
         </oor:items>"
            .to_string()
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 한컴_글꼴을_설치한_한글_글꼴로_잇는다() {
        let items = substitution_items();

        assert!(items.contains("함초롬바탕"), "명조 계열이 없다");
        assert!(items.contains("함초롬돋움"), "고딕 계열이 없다");
        assert!(items.contains(KOREAN_SERIF));
        assert!(items.contains(KOREAN_SANS));
    }

    #[test]
    fn 대체표_사용_플래그를_함께_켠다() {
        // 규칙만 넣고 켜지 않으면 LibreOffice 는 그냥 무시한다 (기본값이 false).
        let items = substitution_items();

        assert!(items.contains("\"Replacement\""));
        assert!(items.contains("<value>true</value>"));
    }

    #[test]
    fn 기존_설정을_지우지_않고_끝에_끼워_넣는다() {
        let registry = empty_registry().replace(
            "</oor:items>",
            "<item oor:path=\"/org.openoffice.Setup\"/></oor:items>",
        );

        let merged = merge_substitutions(&registry).expect("병합 성공");

        assert!(
            merged.contains("/org.openoffice.Setup"),
            "기존 항목이 사라졌다"
        );
        assert!(merged.contains("함초롬바탕"));
        assert!(merged.trim_end().ends_with("</oor:items>"));
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 이미_적용된_레지스트리는_건드리지_않는다() {
        // 변환할 때마다 덧붙이면 파일이 무한히 자란다.
        let once = merge_substitutions(&empty_registry()).expect("첫 병합");

        assert_eq!(merge_substitutions(&once), None);
    }

    #[test]
    fn 알아볼_수_없는_파일은_손대지_않는다() {
        for broken in ["", "그냥 텍스트", "<oor:items>"] {
            assert_eq!(merge_substitutions(broken), None, "{broken:?}");
        }
    }

    #[test]
    fn 세리프와_산세리프를_섞지_않는다() {
        // 본문 명조를 고딕으로 바꾸면 문서 인상이 통째로 달라진다.
        let items = substitution_items();
        let serif_at = items.find("함초롬바탕").expect("명조 규칙");
        let sans_at = items.find("함초롬돋움").expect("고딕 규칙");

        let serif_rule = &items[serif_at..serif_at + 400];
        let sans_rule = &items[sans_at..sans_at + 400];
        assert!(serif_rule.contains(KOREAN_SERIF), "{serif_rule}");
        assert!(sans_rule.contains(KOREAN_SANS), "{sans_rule}");
    }

    #[test]
    fn 규칙_이름이_중복되지_않는다() {
        // 같은 이름을 두 번 넣으면 뒤엣것이 앞엣것을 덮어 규칙이 조용히 사라진다.
        let mut names: Vec<&str> = SERIF_FAMILIES
            .iter()
            .chain(SANS_FAMILIES.iter())
            .copied()
            .collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), total);
        assert_eq!(total, substitution_count());
    }
}
