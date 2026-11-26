//! WhatsNew.cs: release notes shipped in the app (whatsnew.json, newest first, EN and VI), shown after an update;
//! build.bat also turns them into the GitHub notes.
use std::sync::LazyLock;

use serde::Deserialize;

use crate::core::log;

/// `en`: notes in no group (releases before 1.1.1 have only these), and `vi` the same. `highlights`: the release's
/// headline features, shown first (none for most releases). `new`: what is new, `improved` what works better, `fixed`
/// what was wrong and is fixed: each shown under its own heading.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Release {
    pub version: String,
    pub date: String,
    pub en: Option<Vec<String>>,
    pub vi: Option<Vec<String>>,
    pub highlights: Option<Vec<Highlight>>,
    pub new: Option<Notes>,
    pub improved: Option<Notes>,
    pub fixed: Option<Notes>,
    pub removed: Option<Notes>,
}

/// A group of notes, in each language.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Notes {
    pub en: Vec<String>,
    pub vi: Vec<String>,
}

/// A headline feature: an icon (no longer shown), [title, one or two sentences, button] per language, where the
/// button leads ("settings:<page or group>", "guide", "web", "feedback", "discord"; none = no button), and its
/// picture under wwwroot/whatsnew ("{lang}" is en or vi).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Highlight {
    pub icon: String,
    pub go: Option<String>,
    pub en: Vec<String>,
    pub vi: Vec<String>,
    pub image: Option<String>,
}

// PORT: the C# reads whatsnew.json embedded in its assembly with JsonSerializerDefaults.Web (camelCase, any case);
// here the same file is built in, read with camelCase names (exact case, as the file is written).
static RELEASES: LazyLock<Vec<Release>> = LazyLock::new(|| match serde_json::from_str::<Option<Vec<Release>>>(include_str!("../../../RobloxManager/whatsnew.json")) {
    Ok(list) => list.unwrap_or_default(),
    Err(e) => {
        log::write(&e);
        Vec::new()
    }
});

/// System.Version: 2 to 4 numbers; a part not given is -1 (so 1.2 < 1.2.0), and versions compare part by part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: i32,
    pub minor: i32,
    pub build: i32,
    pub revision: i32,
}

impl Version {
    /// Version.TryParse.
    pub fn try_parse(text: &str) -> Option<Version> {
        let parts: Vec<&str> = text.split('.').collect();
        if !(2..=4).contains(&parts.len()) {
            return None;
        }
        let mut n = [-1i32; 4];
        for (i, p) in parts.iter().enumerate() {
            // int.Parse with NumberStyles.Integer (spaces around, a sign), and no part below 0.
            let v: i32 = p.trim().parse().ok()?;
            if v < 0 {
                return None;
            }
            n[i] = v;
        }
        Some(Version { major: n[0], minor: n[1], build: n[2], revision: n[3] })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)?;
        if self.build >= 0 {
            write!(f, ".{}", self.build)?;
            if self.revision >= 0 {
                write!(f, ".{}", self.revision)?;
            }
        }
        Ok(())
    }
}

/// "1.2.3" as a comparable version (a "-beta" or "+build" suffix is ignored); unreadable text is 0.0.
pub fn parse(version: Option<&str>) -> Version {
    let head = version.unwrap_or("").split(['-', '+']).next().unwrap_or("");
    Version::try_parse(head).unwrap_or(Version { major: 0, minor: 0, build: -1, revision: -1 })
}

/// Releases newer than `after` (all when None) up to `up_to`, in the order of `older`.
pub fn between(after: Option<&str>, up_to: &str) -> Vec<Release> {
    RELEASES.iter().filter(|r| after.is_none_or(|a| older(a, &r.version)) && !older(up_to, &r.version)).cloned().collect()
}

/// Notes shown once after an update: the releases in between, none for a beta copy's next beta (it comes in
/// quietly, see `is_quiet_update`).
pub fn after_update(last: &str, current: &str) -> Vec<Release> {
    if !older(last, current) || (is_beta(Some(last)) && is_beta(Some(current)) && parse(Some(last)) == parse(Some(current))) {
        Vec::new()
    } else {
        between(Some(last), current)
    }
}

/// The label after the first '-' (before any '+'), when there is one.
fn label(version: Option<&str>) -> Option<&str> {
    version.unwrap_or("").split('+').next().unwrap_or("").split_once('-').map(|(_, l)| l)
}

/// A quiet fix ("1.0.7-alpha.2"): a fix after a public version (1.0.6) put inside that version's GitHub release
/// (release-fix.ps1), installed in the background without "Restart to update" or notes, shown as the version it
/// fixes. Numbered below the next version's betas so they still update over it.
pub fn is_quiet_fix(version: Option<&str>) -> bool {
    label(version).is_some_and(|l| l.starts_with("alpha."))
}

pub fn is_beta(version: Option<&str>) -> bool {
    label(version).is_some_and(|l| l.starts_with("beta."))
}

/// An update that comes in quietly, without "Restart to update": a quiet fix, or the next beta of the version a beta
/// copy is on, whose notes don't show either (`since`).
pub fn is_quiet_update(next: Option<&str>, current: Option<&str>) -> bool {
    is_quiet_fix(next) || (is_beta(current) && is_beta(next) && parse(next) == parse(current))
}

/// Velopack's order: a number's quiet fixes, then its betas (and other labels), then the version itself. A test
/// build between two fixes (1.0.8-alpha.3.1) counts as the first. Until 2026-10-25 a beta and its version counted as
/// the same, so 1.0.10-beta.3 → 1.0.10 showed no notes.
pub fn older(a: &str, b: &str) -> bool {
    fn key(v: &str) -> (Version, i32, i32) {
        let label = label(Some(v)).unwrap_or("");
        let kind = if label.is_empty() {
            3
        } else if is_quiet_fix(Some(v)) {
            0
        } else if is_beta(Some(v)) {
            1
        } else {
            2
        };
        let parts: Vec<&str> = label.split('.').collect();
        let n = if kind < 3 && parts.len() >= 2 { parts[1].trim().parse::<i32>().unwrap_or(i32::MAX) } else { i32::MAX };
        (parse(Some(v)), kind, n)
    }
    let (x, y) = (key(a), key(b));
    x.0 < y.0 || (x.0 == y.0 && (x.1 < y.1 || (x.1 == y.1 && x.2 < y.2)))
}

/// The version as people see it: a quiet fix shows as the public version it fixes (1.0.7-alpha.2 is 1.0.6).
pub fn shown(version: &str) -> String {
    if !is_quiet_fix(Some(version)) {
        return version.to_string();
    }
    RELEASES
        .iter()
        .map(|r| r.version.as_str())
        .find(|v| !v.contains('-') && parse(Some(v)) < parse(Some(version)))
        .unwrap_or(version)
        .to_string()
}
