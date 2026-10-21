//! Distribution detection based on the freedesktop `os-release` standard.
//!
//! Detection relies exclusively on `ID` and `ID_LIKE` fields, which is the
//! portable mechanism distributions themselves use to declare inheritance.
//! Nothing here keys off filesystem layout, package manager, or shell.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// A distribution family that DotPilot recognises.
///
/// Families group distributions that share filesystem and tooling conventions.
/// The variants are intentionally coarse: finer distinctions belong in
/// [`Distro`] metadata rather than in branching logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Family {
    /// Arch Linux and derivatives (Manjaro, EndeavourOS, Garuda, ...).
    Arch,
    /// Debian and derivatives (Ubuntu, Mint, Pop!_OS, elementary, ...).
    Debian,
    /// Fedora and derivatives (Nobara, Bazzite, ...).
    Fedora,
    /// openSUSE and derivatives (Leap, Tumbleweed, MicroOS, ...).
    Suse,
    /// NixOS and other Nix-based systems.
    Nix,
    /// Gentoo and derivatives.
    Gentoo,
    /// Alpine Linux and derivatives.
    Alpine,
    /// A recognised-but-unclassified distribution.
    Other,
    /// The distribution could not be identified.
    Unknown,
}

impl Family {
    /// Stable machine-readable identifier, used by the frontend and persisted
    /// in profile metadata.
    pub fn id(self) -> &'static str {
        match self {
            Family::Arch => "arch",
            Family::Debian => "debian",
            Family::Fedora => "fedora",
            Family::Suse => "suse",
            Family::Nix => "nix",
            Family::Gentoo => "gentoo",
            Family::Alpine => "alpine",
            Family::Other => "other",
            Family::Unknown => "unknown",
        }
    }

    /// Human readable label.
    pub fn label(self) -> &'static str {
        match self {
            Family::Arch => "Arch-based",
            Family::Debian => "Debian-based",
            Family::Fedora => "Fedora-based",
            Family::Suse => "openSUSE-based",
            Family::Nix => "Nix-based",
            Family::Gentoo => "Gentoo-based",
            Family::Alpine => "Alpine-based",
            Family::Other => "Other",
            Family::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Distribution metadata parsed from `os-release`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Distro {
    /// The `ID` field, e.g. `ubuntu`. Lowercase.
    pub id: String,
    /// The `ID_LIKE` field split on whitespace, e.g. `["ubuntu", "debian"]`.
    pub id_like: Vec<String>,
    /// The `NAME` field, e.g. `Ubuntu`.
    pub name: String,
    /// The `VERSION_ID` field, e.g. `24.04`.
    pub version_id: Option<String>,
    /// The `VERSION_CODENAME` field, e.g. `noble`.
    pub version_codename: Option<String>,
    /// The resolved distribution family.
    pub family: Family,
    /// The raw `PRETTY_NAME` field, when present.
    pub pretty_name: Option<String>,
}

impl Distro {
    /// Detect the running distribution.
    ///
    /// Reads `/etc/os-release`, falling back to `/usr/lib/os-release`. Both
    /// paths are part of the freedesktop standard; the first is conventionally
    /// a symlink to the second, but immutable and minimal images ship only one
    /// of them.
    pub fn detect() -> Distro {
        for path in OS_RELEASE_PATHS {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Some(distro) = Distro::from_os_release(&text) {
                    return distro;
                }
            }
        }
        Distro::unknown()
    }

    /// Parse the contents of an `os-release` file.
    ///
    /// Returns `None` when the file contains no `ID` field, which means it is
    /// not a usable `os-release` document.
    pub fn from_os_release(text: &str) -> Option<Distro> {
        let fields = parse_os_release(text);
        let id = fields.get("ID").map(|v| v.to_ascii_lowercase())?;
        if id.is_empty() {
            return None;
        }
        let id_like = fields
            .get("ID_LIKE")
            .map(|v| {
                v.split_whitespace()
                    .map(|s| s.to_ascii_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        let family = resolve_family(&id, &id_like);
        let name = fields
            .get("NAME")
            .map(|v| v.to_string())
            .unwrap_or_else(|| id.clone());

        Some(Distro {
            family,
            id,
            id_like,
            name,
            version_id: fields.get("VERSION_ID").map(|v| v.to_string()),
            version_codename: fields.get("VERSION_CODENAME").map(|v| v.to_string()),
            pretty_name: fields.get("PRETTY_NAME").map(|v| v.to_string()),
        })
    }

    /// A placeholder used when no `os-release` file can be read.
    pub fn unknown() -> Distro {
        Distro {
            id: "unknown".to_string(),
            id_like: Vec::new(),
            name: "Unknown".to_string(),
            version_id: None,
            version_codename: None,
            family: Family::Unknown,
            pretty_name: None,
        }
    }

    /// A short human readable description, e.g. `Ubuntu 24.04 (noble)`.
    pub fn describe(&self) -> String {
        let mut out = match &self.version_id {
            Some(v) => format!("{} {}", self.name, v),
            None => self.name.clone(),
        };
        if let Some(codename) = &self.version_codename {
            out.push_str(&format!(" ({})", codename));
        }
        out
    }
}

/// Candidate `os-release` locations, in priority order.
pub const OS_RELEASE_PATHS: [&str; 2] = ["/etc/os-release", "/usr/lib/os-release"];

/// Map of known distribution IDs to their family.
///
/// Keys are matched against `ID` first, then against each `ID_LIKE` entry in
/// order. Because `ID_LIKE` is the standard inheritance mechanism, a
/// distribution that is not listed here still resolves correctly as long as
/// it names a parent that is (e.g. `ID_LIKE=arch` -> Arch).
fn family_by_id(id: &str) -> Option<Family> {
    Some(match id {
        // Arch family.
        "arch" | "archarm" | "manjaro" | "endeavouros" | "garuda" | "artix" | "cachyos" => {
            Family::Arch
        }
        // Debian family. `ubuntu` and `mint` are listed explicitly because
        // they are commonly referenced as parents by other distributions.
        "debian" | "ubuntu" | "linuxmint" | "mint" | "pop" | "raspbian" | "devuan"
        | "kali" | "parrot" | "elementary" | "zorin" | "neon" => Family::Debian,
        // Fedora family. `rhel` is included because Fedora derivatives such as
        // Nobara and Bazzite commonly declare `ID_LIKE=fedora` or `rhel`.
        "fedora" | "nobara" | "rhel" | "centos" | "rocky" | "almalinux" => Family::Fedora,
        // SUSE family.
        "opensuse" | "opensuse-leap" | "opensuse-tumbleweed" | "opensuse-microos" | "sles"
        | "sled" | "suse" => Family::Suse,
        // Nix family.
        "nixos" | "nix" => Family::Nix,
        // Gentoo family.
        "gentoo" | "funtoo" | "calculate" => Family::Gentoo,
        // Alpine family.
        "alpine" | "postmarketos" => Family::Alpine,
        _ => return None,
    })
}

/// Resolve a family from `ID` and `ID_LIKE`.
///
/// `ID` wins when it is directly known. Otherwise each `ID_LIKE` entry is
/// tried in declaration order, so `ID_LIKE="ubuntu debian"` resolves to the
/// Debian family.
fn resolve_family(id: &str, id_like: &[String]) -> Family {
    if let Some(family) = family_by_id(id) {
        return family;
    }
    for parent in id_like {
        if let Some(family) = family_by_id(parent) {
            return family;
        }
    }
    if id_like.is_empty() && id.is_empty() {
        Family::Unknown
    } else {
        Family::Other
    }
}

/// Parse an `os-release` document into a map of keys to values.
///
/// Values may be quoted with single or double quotes and may contain escaped
/// characters, matching the shell-compatible syntax mandated by the standard.
fn parse_os_release(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        map.insert(key.to_string(), unquote_os_release_value(raw_value.trim()));
    }
    map
}

/// Strip surrounding quotes and resolve escape sequences in an `os-release`
/// value.
fn unquote_os_release_value(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        return unescape(&value[1..value.len() - 1], true);
    }
    if bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
        // Single quotes are literal in the standard.
        return value[1..value.len() - 1].to_string();
    }
    value.to_string()
}

/// Resolve backslash escapes in a double-quoted `os-release` value.
fn unescape(value: &str, double_quoted: bool) -> String {
    if !double_quoted {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('$') => out.push('$'),
            Some('`') => out.push('`'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Returns the path of the `os-release` file that was used, if any.
///
/// Useful for diagnostics in the UI: it tells the user which file the
/// detection came from, which is the first question when a distribution is
/// misidentified.
pub fn os_release_path() -> Option<PathBuf> {
    for path in OS_RELEASE_PATHS {
        let path = Path::new(path);
        if path.is_file() {
            return Some(path.to_path_buf());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_arch_os_release() {
        let text = r#"
NAME="Arch Linux"
PRETTY_NAME="Arch Linux"
ID=arch
BUILD_ID=rolling
ANSI_COLOR="38;2;23;147;209"
"#;
        let distro = Distro::from_os_release(text).expect("should parse");
        assert_eq!(distro.id, "arch");
        assert_eq!(distro.family, Family::Arch);
        assert_eq!(distro.name, "Arch Linux");
        assert!(distro.id_like.is_empty());
    }

    #[test]
    fn parses_quoted_values_and_id_like() {
        let text = r#"
NAME="Ubuntu"
VERSION="24.04.1 LTS (Noble Numbat)"
ID=ubuntu
ID_LIKE="ubuntu debian"
VERSION_ID="24.04"
VERSION_CODENAME=noble
"#;
        let distro = Distro::from_os_release(text).expect("should parse");
        assert_eq!(distro.id, "ubuntu");
        assert_eq!(distro.id_like, vec!["ubuntu", "debian"]);
        assert_eq!(distro.family, Family::Debian);
        assert_eq!(distro.version_id.as_deref(), Some("24.04"));
        assert_eq!(distro.version_codename.as_deref(), Some("noble"));
        assert_eq!(distro.name, "Ubuntu");
    }

    #[test]
    fn resolves_derivatives_through_id_like() {
        // EndeavourOS declares `ID=endeavouros` with `ID_LIKE=arch`.
        let text = "ID=endeavouros\nID_LIKE=arch\nNAME=\"EndeavourOS\"\n";
        let distro = Distro::from_os_release(text).expect("should parse");
        assert_eq!(distro.family, Family::Arch);

        // Manjaro is directly known.
        let text = "ID=manjaro\nID_LIKE=arch\n";
        assert_eq!(
            Distro::from_os_release(text).unwrap().family,
            Family::Arch
        );

        // Pop!_OS is not directly known but inherits from ubuntu.
        let text = "ID=pop\nID_LIKE=\"ubuntu debian\"\n";
        assert_eq!(
            Distro::from_os_release(text).unwrap().family,
            Family::Debian
        );

        // Nobara inherits from fedora.
        let text = "ID=nobra\nID_LIKE=fedora\n";
        assert_eq!(
            Distro::from_os_release(text).unwrap().family,
            Family::Fedora
        );
    }

    #[test]
    fn resolves_opensuse_variants() {
        for id in ["opensuse-leap", "opensuse-tumbleweed", "opensuse-microos"] {
            let text = format!("ID={}\nID_LIKE=\"suse opensuse\"\n", id);
            let distro = Distro::from_os_release(&text).unwrap();
            assert_eq!(distro.family, Family::Suse, "id {}", id);
        }
    }

    #[test]
    fn resolves_nix_gentoo_and_alpine() {
        assert_eq!(
            Distro::from_os_release("ID=nixos\n").unwrap().family,
            Family::Nix
        );
        assert_eq!(
            Distro::from_os_release("ID=gentoo\n").unwrap().family,
            Family::Gentoo
        );
        assert_eq!(
            Distro::from_os_release("ID=alpine\n").unwrap().family,
            Family::Alpine
        );
    }

    #[test]
    fn unknown_distribution_is_other_not_unknown() {
        let distro = Distro::from_os_release("ID=weirdlinux\n").unwrap();
        assert_eq!(distro.family, Family::Other);
    }

    #[test]
    fn missing_id_yields_none() {
        assert!(Distro::from_os_release("NAME=\"No ID\"\n").is_none());
        assert!(Distro::from_os_release("").is_none());
    }

    #[test]
    fn id_wins_over_id_like() {
        // `ID=ubuntu` is Debian family even though `ID_LIKE` names debian.
        let distro = Distro::from_os_release("ID=ubuntu\nID_LIKE=debian\n").unwrap();
        assert_eq!(distro.family, Family::Debian);
    }

    #[test]
    fn single_quoted_values_are_literal() {
        let distro = Distro::from_os_release("ID=arch\nNAME='Arch \"quoted\"'\n").unwrap();
        assert_eq!(distro.name, "Arch \"quoted\"");
    }

    #[test]
    fn double_quoted_values_unescape() {
        let distro = Distro::from_os_release("ID=arch\nNAME=\"a\\\"b\\\\c\"\n").unwrap();
        assert_eq!(distro.name, "a\"b\\c");
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let distro = Distro::from_os_release(
            "# a comment\n\nID=fedora\n   \n# trailing\nNAME=\"Fedora\"\n",
        )
        .unwrap();
        assert_eq!(distro.id, "fedora");
        assert_eq!(distro.name, "Fedora");
    }

    #[test]
    fn describe_includes_version_and_codename() {
        let distro = Distro::from_os_release(
            "ID=ubuntu\nNAME=Ubuntu\nVERSION_ID=24.04\nVERSION_CODENAME=noble\n",
        )
        .unwrap();
        assert_eq!(distro.describe(), "Ubuntu 24.04 (noble)");
    }
}
