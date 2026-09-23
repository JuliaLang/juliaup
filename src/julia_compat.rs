//! Pkg's `[compat]` version specifiers, and upgrade offers based on the
//! `julia` compat entry of a project.
//!
//! This is a port of Pkg's `Versions.jl` (`semver_spec`), see
//! https://pkgdocs.julialang.org/v1/compatibility/

use anyhow::{bail, Context, Result};
use regex::{Captures, Regex};
use semver::Version;
use std::sync::OnceLock;

/// A version bound with `n` significant components (Pkg's `VersionBound`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VersionBound {
    t: [u64; 3],
    n: usize,
}

impl VersionBound {
    fn new(parts: &[u64]) -> Self {
        let mut t = [0; 3];
        t[..parts.len()].copy_from_slice(parts);
        VersionBound { t, n: parts.len() }
    }

    fn unbounded() -> Self {
        VersionBound { t: [0; 3], n: 0 }
    }

    fn significant(&self, v: &Version) -> ([u64; 3], [u64; 3]) {
        let mut a = [0; 3];
        let mut b = [0; 3];
        let version = [v.major, v.minor, v.patch];
        a[..self.n].copy_from_slice(&version[..self.n]);
        b[..self.n].copy_from_slice(&self.t[..self.n]);
        (a, b)
    }

    /// `v ≲ self` for an upper bound
    fn is_above(&self, v: &Version) -> bool {
        let (v, b) = self.significant(v);
        v <= b
    }

    /// `self ≲ v` for a lower bound
    fn is_below(&self, v: &Version) -> bool {
        let (v, b) = self.significant(v);
        v >= b
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VersionRange {
    lower: VersionBound,
    upper: VersionBound,
}

impl VersionRange {
    fn contains(&self, v: &Version) -> bool {
        self.lower.is_below(v) && self.upper.is_above(v)
    }
}

/// A parsed `[compat]` entry (Pkg's `VersionSpec`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSpec {
    ranges: Vec<VersionRange>,
}

fn regexes() -> &'static [Regex; 3] {
    static REGEXES: OnceLock<[Regex; 3]> = OnceLock::new();
    REGEXES.get_or_init(|| {
        let version = r"v?([0-9]+)(?:\.([0-9]+))?(?:\.([0-9]+))?";
        [
            // 0.5 ^0.4 ~0.3.2
            Regex::new(&format!(r"^([~^]?){version}$")).unwrap(),
            // < 0.2 >= 0.5 = 1.2.3
            Regex::new(&format!(
                r"^((?:≥\s*)|(?:>=\s*)|(?:=\s*)|(?:<\s*))v?{version}$"
            ))
            .unwrap(),
            // 0.7 - 1.3
            Regex::new(&format!(r"^\s*{version}\s*?\s-\s\s*?{version}\s*$")).unwrap(),
        ]
    })
}

/// The captured version components starting at capture group `first`.
fn captured_parts(caps: &Captures, first: usize) -> Result<Vec<u64>> {
    (first..first + 3)
        .map_while(|i| caps.get(i))
        .map(|m| {
            m.as_str()
                .parse::<u64>()
                .with_context(|| format!("Invalid version number `{}`.", m.as_str()))
        })
        .collect()
}

fn full_version(parts: &[u64]) -> [u64; 3] {
    let mut v = [0; 3];
    v[..parts.len()].copy_from_slice(parts);
    v
}

fn semver_interval(typ: &str, parts: &[u64]) -> Result<VersionRange> {
    let [major, minor, patch] = full_version(parts);
    if parts.len() == 3 && major == 0 && minor == 0 && patch == 0 {
        bail!("Invalid version: \"0.0.0\".");
    }
    let lower = VersionBound::new(&[major, minor, patch]);
    let upper = if typ.is_empty() || typ == "^" {
        // Caret specifier (the default)
        if major != 0 {
            VersionBound::new(&[major])
        } else if minor != 0 {
            VersionBound::new(&[major, minor])
        } else {
            match parts.len() {
                1 => VersionBound::new(&[0]),
                2 => VersionBound::new(&[0, 0]),
                _ => VersionBound::new(&[0, 0, patch]),
            }
        }
    } else if parts.len() >= 2 {
        // Tilde specifier
        VersionBound::new(&[major, minor])
    } else {
        VersionBound::new(&[major])
    };
    Ok(VersionRange { lower, upper })
}

fn inequality_interval(typ: &str, parts: &[u64]) -> Result<VersionRange> {
    let [major, minor, patch] = full_version(parts);
    if parts.len() == 3 && major == 0 && minor == 0 && patch == 0 {
        bail!("Invalid version: \"0.0.0\".");
    }
    let v = VersionBound::new(&[major, minor, patch]);
    let typ = typ.trim();
    Ok(match typ {
        "<" => {
            let upper = if patch == 0 {
                if minor == 0 {
                    if major == 0 {
                        bail!("Invalid version specifier: \"< 0\".");
                    }
                    VersionBound::new(&[major - 1])
                } else {
                    VersionBound::new(&[major, minor - 1])
                }
            } else {
                VersionBound::new(&[major, minor, patch - 1])
            };
            VersionRange {
                lower: VersionBound::new(&[0, 0, 0]),
                upper,
            }
        }
        "=" => VersionRange { lower: v, upper: v },
        ">=" | "≥" => VersionRange {
            lower: v,
            upper: VersionBound::unbounded(),
        },
        _ => bail!("Invalid prefix `{}`.", typ),
    })
}

impl VersionSpec {
    /// Parse a compat entry such as `"1.6"`, `"~1.10, ~1.11"` or `"1.6 - 1.9"`.
    pub fn parse(spec: &str) -> Result<Self> {
        let [semver_re, inequality_re, hyphen_re] = regexes();
        let mut ranges = Vec::new();
        for part in spec.trim().split(',').map(str::trim) {
            let range = if let Some(caps) = semver_re.captures(part) {
                semver_interval(&caps[1], &captured_parts(&caps, 2)?)?
            } else if let Some(caps) = inequality_re.captures(part) {
                inequality_interval(&caps[1], &captured_parts(&caps, 2)?)?
            } else if let Some(caps) = hyphen_re.captures(part) {
                let lower = captured_parts(&caps, 1)?;
                let upper = captured_parts(&caps, 4)?;
                VersionRange {
                    lower: VersionBound::new(&lower),
                    upper: VersionBound::new(&upper),
                }
            } else {
                bail!("Invalid version specifier: \"{}\".", spec);
            };
            ranges.push(range);
        }
        Ok(VersionSpec { ranges })
    }

    /// Whether `version` satisfies this specifier. Prerelease and build
    /// information is ignored, as in Pkg.
    pub fn contains(&self, version: &Version) -> bool {
        self.ranges.iter().any(|range| range.contains(version))
    }
}

/// Julia versions a project could be moved to, based on its `julia` compat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeOffer {
    /// The newest version the project's compat allows.
    pub latest: Version,
    /// The newest version with the same minor version as the current one, if
    /// it is allowed, newer than the current version and different from `latest`.
    pub latest_patch: Option<Version>,
    /// Whether the current version is not allowed by the project's compat.
    pub current_violates_compat: bool,
}

/// Determine whether to offer moving a project from `current` to another Julia
/// version.
///
/// `compat` holds the `julia` compat entries of all projects in the workspace;
/// a version must satisfy all of them. Without any compat entry, only newer
/// patch versions of the current minor version are offered. `available` are the
/// stable Julia versions to choose from.
pub fn compute_upgrade_offer(
    current: &Version,
    compat: &[VersionSpec],
    available: &[Version],
) -> Option<UpgradeOffer> {
    let same_minor = |v: &Version| v.major == current.major && v.minor == current.minor;
    let allowed = |v: &Version| {
        if compat.is_empty() {
            same_minor(v)
        } else {
            compat.iter().all(|spec| spec.contains(v))
        }
    };

    let current_violates_compat = !compat.is_empty() && !allowed(current);

    let candidates: Vec<&Version> = available
        .iter()
        .filter(|v| v.pre.is_empty() && allowed(v))
        .filter(|v| current_violates_compat || *v > current)
        .collect();

    let latest = (*candidates.iter().max()?).clone();
    let latest_patch = candidates
        .iter()
        .filter(|v| same_minor(v) && **v > current)
        .max()
        .map(|v| (*v).clone())
        .filter(|v| *v != latest);

    Some(UpgradeOffer {
        latest,
        latest_patch,
        current_violates_compat,
    })
}
