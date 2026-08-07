//! 런타임 자산 pin 테이블.
//!
//! URL 과 sha256 은 **빌드 시점에 고정**한다. 런타임에 Adoptium API 같은 메타데이터
//! 엔드포인트를 조회하면 응답이 바뀌는 순간 pin 한 해시와 어긋나 설치가 통째로 막힌다.
//! 새 버전으로 올릴 때는 사람이 해시를 다시 확인해 이 표를 고친다.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetSpec {
    pub url: &'static str,
    pub sha256: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Os {
    MacOs,
    Windows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Arch {
    Aarch64,
    X86_64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

pub const LO_VERSION: &str = "26.2.5";
pub const H2O_VERSION: &str = "0.7.13";
pub const JRE_VERSION: &str = "21.0.12+8";
/// `unopkg list` 에서 우리 확장을 골라내는 키.
pub const H2O_IDENTIFIER: &str = "ebandal.libreoffice.H2Orestart";

impl Os {
    /// Temurin 릴리스 파일명에 쓰이는 OS 토큰.
    pub fn temurin_token(&self) -> &'static str {
        match self {
            Os::MacOs => "mac",
            Os::Windows => "windows",
        }
    }

    /// Temurin 배포 아카이브 확장자.
    pub fn archive_extension(&self) -> &'static str {
        match self {
            Os::MacOs => "tar.gz",
            Os::Windows => "zip",
        }
    }
}

impl Arch {
    /// Temurin 릴리스 파일명에 쓰이는 아키텍처 토큰.
    pub fn temurin_token(&self) -> &'static str {
        match self {
            Arch::Aarch64 => "aarch64",
            Arch::X86_64 => "x64",
        }
    }
}

impl Platform {
    pub fn new(os: Os, arch: Arch) -> Self {
        Self { os, arch }
    }

    /// 현재 호스트. 지원하지 않는 조합(리눅스 등)이면 None.
    pub fn host() -> Option<Self> {
        let os = if cfg!(target_os = "macos") {
            Os::MacOs
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            return None;
        };

        let arch = if cfg!(target_arch = "aarch64") {
            Arch::Aarch64
        } else if cfg!(target_arch = "x86_64") {
            Arch::X86_64
        } else {
            return None;
        };

        Some(Self::new(os, arch))
    }
}

/// LibreOffice 26.2.5 공식 배포본 (The Document Foundation).
pub fn libreoffice_asset(platform: Platform) -> AssetSpec {
    match (platform.os, platform.arch) {
        (Os::MacOs, Arch::Aarch64) => AssetSpec {
            url: "https://download.documentfoundation.org/libreoffice/stable/26.2.5/mac/aarch64/\
                  LibreOffice_26.2.5_MacOS_aarch64.dmg",
            sha256: "c99fb4fe574437fc4cb820a4ca15271bca325920861f7139858b36d7f9df78ad",
        },
        (Os::MacOs, Arch::X86_64) => AssetSpec {
            url: "https://download.documentfoundation.org/libreoffice/stable/26.2.5/mac/x86_64/\
                  LibreOffice_26.2.5_MacOS_x86-64.dmg",
            sha256: "e26180298685274b54aa7fe6e1101c65465a372f457a6748ebd642720811db36",
        },
        (Os::Windows, Arch::X86_64) => AssetSpec {
            url: "https://download.documentfoundation.org/libreoffice/stable/26.2.5/win/x86_64/\
                  LibreOffice_26.2.5_Win_x86-64.msi",
            sha256: "f15ba07bfcb0186986cf3171063506f5d207c11f8cc051ba0d135209e9e915f9",
        },
        (Os::Windows, Arch::Aarch64) => AssetSpec {
            url: "https://download.documentfoundation.org/libreoffice/stable/26.2.5/win/aarch64/\
                  LibreOffice_26.2.5_Win_aarch64.msi",
            sha256: "48e99bba813c65a823b86a9fe8c0746a415f3d0e9459255f81f745f58fd353aa",
        },
    }
}

/// Eclipse Temurin JRE 21.0.12+8 — H2Orestart 가 Java 확장이라 반드시 필요하다.
pub fn jre_asset(platform: Platform) -> AssetSpec {
    match (platform.os, platform.arch) {
        (Os::MacOs, Arch::Aarch64) => AssetSpec {
            url: "https://github.com/adoptium/temurin21-binaries/releases/download/\
                  jdk-21.0.12%2B8/OpenJDK21U-jre_aarch64_mac_hotspot_21.0.12_8.tar.gz",
            sha256: "36bb71d6fa5184e12a6483e7662783c2cbd383f5dca8034140f0a84dd5aa797d",
        },
        (Os::MacOs, Arch::X86_64) => AssetSpec {
            url: "https://github.com/adoptium/temurin21-binaries/releases/download/\
                  jdk-21.0.12%2B8/OpenJDK21U-jre_x64_mac_hotspot_21.0.12_8.tar.gz",
            sha256: "539706197baea8189c9a677aea5bf44671b74a71baa42dde436e312f2158fa3a",
        },
        (Os::Windows, Arch::X86_64) => AssetSpec {
            url: "https://github.com/adoptium/temurin21-binaries/releases/download/\
                  jdk-21.0.12%2B8/OpenJDK21U-jre_x64_windows_hotspot_21.0.12_8.zip",
            sha256: "b8aa18fef5edb69bee8618f99677d66d0873d22cb40d974c15ac9ffcdecf73ba",
        },
        (Os::Windows, Arch::Aarch64) => AssetSpec {
            url: "https://github.com/adoptium/temurin21-binaries/releases/download/\
                  jdk-21.0.12%2B8/OpenJDK21U-jre_aarch64_windows_hotspot_21.0.12_8.zip",
            sha256: "a50ed83b6a88d3127d406713f5057d78f845c3412d59e201dac6db37714af85c",
        },
    }
}

/// H2Orestart 0.7.13 확장 (GPLv3) — 코드로 링크하지 않고 unopkg 로만 설치한다.
pub fn h2orestart_asset() -> AssetSpec {
    AssetSpec {
        url: "https://github.com/ebandal/H2Orestart/releases/download/v0.7.13/H2Orestart.oxt",
        sha256: "726230215dabe450bd617f9acac52376fd76f57c77158bd03b3ef9fe0c7e64fd",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_PLATFORMS: [Platform; 4] = [
        Platform {
            os: Os::MacOs,
            arch: Arch::Aarch64,
        },
        Platform {
            os: Os::MacOs,
            arch: Arch::X86_64,
        },
        Platform {
            os: Os::Windows,
            arch: Arch::X86_64,
        },
        Platform {
            os: Os::Windows,
            arch: Arch::Aarch64,
        },
    ];

    fn is_lower_hex64(value: &str) -> bool {
        value.len() == 64
            && value
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 자산표가_네_플랫폼을_모두_덮는다() {
        let mut urls: Vec<&str> = ALL_PLATFORMS
            .iter()
            .flat_map(|p| [libreoffice_asset(*p).url, jre_asset(*p).url])
            .collect();
        let count = urls.len();
        urls.sort_unstable();
        urls.dedup();

        assert_eq!(urls.len(), count, "플랫폼별 자산 URL 이 중복되면 안 된다");
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 모든_sha256_은_64자_소문자_hex_다() {
        for platform in ALL_PLATFORMS {
            assert!(is_lower_hex64(libreoffice_asset(platform).sha256));
            assert!(is_lower_hex64(jre_asset(platform).sha256));
        }

        assert!(is_lower_hex64(h2orestart_asset().sha256));
    }

    #[test]
    fn libreoffice_url_에_pin_한_버전과_확장자가_들어_있다() {
        for platform in ALL_PLATFORMS {
            let spec = libreoffice_asset(platform);

            assert!(spec.url.contains(LO_VERSION), "{}", spec.url);
            match platform.os {
                Os::MacOs => assert!(spec.url.ends_with(".dmg"), "{}", spec.url),
                Os::Windows => assert!(spec.url.ends_with(".msi"), "{}", spec.url),
            }
        }
    }

    #[test]
    fn jre_url_이_arch_os_조합대로_조립된다() {
        for platform in ALL_PLATFORMS {
            let expected = format!(
                "https://github.com/adoptium/temurin21-binaries/releases/download/\
                 jdk-21.0.12%2B8/OpenJDK21U-jre_{}_{}_hotspot_21.0.12_8.{}",
                platform.arch.temurin_token(),
                platform.os.temurin_token(),
                platform.os.archive_extension(),
            );

            assert_eq!(jre_asset(platform).url, expected);
        }
    }

    #[test]
    fn h2orestart_는_버전과_식별자를_pin_한다() {
        let spec = h2orestart_asset();

        assert!(spec.url.contains(H2O_VERSION), "{}", spec.url);
        assert!(spec.url.ends_with(".oxt"), "{}", spec.url);
        assert_eq!(H2O_IDENTIFIER, "ebandal.libreoffice.H2Orestart");
        assert_eq!(JRE_VERSION, "21.0.12+8");
    }

    #[test]
    fn host_는_지원_플랫폼에서만_some_이다() {
        let host = Platform::host();

        if cfg!(target_os = "macos") {
            assert_eq!(host.map(|p| p.os), Some(Os::MacOs));
        } else if cfg!(target_os = "windows") {
            assert_eq!(host.map(|p| p.os), Some(Os::Windows));
        } else {
            assert_eq!(host, None, "리눅스 등 미지원 호스트는 None 이어야 한다");
        }

        if let Some(platform) = host {
            // 호스트가 정해졌다면 자산표에도 반드시 항목이 있어야 한다.
            assert!(!libreoffice_asset(platform).url.is_empty());
            assert!(!jre_asset(platform).url.is_empty());
        }
    }
}
