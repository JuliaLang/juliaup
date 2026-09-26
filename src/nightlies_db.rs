//! The catalog of nightly builds, `nightlies.json`.
//!
//! Maps nightly channel names to standard and variant artifacts. Unknown JSON
//! fields are ignored; only supported target triplets and archive kinds are
//! selected. A cached copy lives next to the versions database.

use crate::channel_name::{ChannelBase, ChannelName};
use crate::global_paths::GlobalPaths;
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::time::{Duration, Instant};

/// One downloadable file of a nightly channel.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NightlyFile {
    /// `linux`, `mac`, `winnt` or `freebsd`.
    pub os: String,
    /// `x86_64`, `i686` or `aarch64`.
    pub arch: String,
    pub triplet: String,
    /// `archive` or `installer`.
    pub kind: String,
    /// `tar.gz`, `dmg`, `zip` or `exe`.
    pub extension: String,
    pub url: String,
    /// The build variants applied to this file (`opt`, `assert`, `nogpl`,
    /// ...); empty for the standard build.
    #[serde(default)]
    pub variants: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct NightlyChannel {
    /// The standard builds.
    pub files: Vec<NightlyFile>,
    /// Build variants of the standard builds.
    #[serde(default)]
    pub variants: Vec<NightlyFile>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(transparent)]
pub struct NightliesDb {
    pub channels: BTreeMap<String, NightlyChannel>,
}

/// A platform in the vocabulary of `nightlies.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Platform {
    pub os: &'static str,
    pub arch: &'static str,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.os, self.arch)
    }
}

impl Platform {
    /// The platform juliaup is running on, with `arch` given as a juliaup arch
    /// identifier (`x64`, `x86`, `aarch64`).
    pub fn current(arch: &str) -> Result<Self> {
        let os = if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "macos") {
            "mac"
        } else if cfg!(target_os = "windows") {
            "winnt"
        } else if cfg!(target_os = "freebsd") {
            "freebsd"
        } else {
            bail!("Unsupported operating system for nightly channels.")
        };
        let arch = match arch {
            "x64" => "x86_64",
            "x86" => "i686",
            "aarch64" => "aarch64",
            _ => bail!("Unsupported architecture `{}` for nightly channels.", arch),
        };
        Ok(Platform { os, arch })
    }
}

impl NightlyFile {
    fn has_valid_variants(&self) -> bool {
        let sorted = self.sorted_variants();
        sorted
            .iter()
            .all(|v| !v.is_empty() && v.bytes().all(|c| c.is_ascii_alphanumeric()))
            && !sorted.windows(2).any(|pair| pair[0] == pair[1])
    }

    fn is_for(&self, platform: Platform) -> bool {
        self.os == platform.os
            && self.arch == platform.arch
            && matches!(
                (platform.os, platform.arch, self.triplet.as_str()),
                ("linux", "x86_64", "x86_64-linux-gnu")
                    | ("linux", "i686", "i686-linux-gnu")
                    | ("linux", "aarch64", "aarch64-linux-gnu")
                    | ("mac", "x86_64", "x86_64-apple-darwin14")
                    | ("mac", "aarch64", "aarch64-apple-darwin14")
                    | ("winnt", "x86_64", "x86_64-w64-mingw32")
                    | ("winnt", "i686", "i686-w64-mingw32")
                    | ("freebsd", "x86_64", "x86_64-unknown-freebsd11.1")
            )
    }

    /// Match the variant order used by `ChannelName`.
    fn sorted_variants(&self) -> Vec<String> {
        let mut variants = self.variants.clone();
        variants.sort();
        variants
    }

    /// juliaup installs from the tarball on every platform. macOS additionally
    /// tries the DMG derived from the tarball URL first (see
    /// `install_from_url`), so the tarball URL stays the canonical one.
    fn is_installable(&self) -> bool {
        self.kind == "archive" && self.extension == "tar.gz"
    }

    /// The file name without the `julia-` prefix and extension, e.g.
    /// `latest-linux-x86_64`: the closest thing a nightly has to a version.
    pub fn label(&self) -> String {
        let name = self.url.rsplit('/').next().unwrap_or(&self.url);
        let name = name.strip_prefix("julia-").unwrap_or(name);
        name.strip_suffix(&format!(".{}", self.extension))
            .unwrap_or(name)
            .to_string()
    }
}

impl NightliesDb {
    pub fn parse(json: &str) -> Result<Self> {
        let mut db: Self = serde_json::from_str(json).context("Failed to parse nightlies.json.")?;
        // Unknown channel syntax must not break listing or manifest selection.
        db.channels.retain(|base, _| {
            ChannelName::parse(base).is_ok_and(|name| {
                name.is_nightly()
                    && name.variants.is_empty()
                    && name.arch.is_none()
                    && name.to_string() == *base
            })
        });
        Ok(db)
    }

    pub fn lookup(&self, channel: &ChannelName) -> Result<&NightlyFile> {
        let arch = channel
            .arch
            .clone()
            .map_or_else(crate::operations::default_arch, Ok)?;
        self.select(channel, Platform::current(&arch)?)
    }

    pub fn has_standard_build(&self, base: &str) -> bool {
        ChannelName::parse(base).is_ok_and(|name| self.lookup(&name).is_ok())
    }

    pub fn rows(&self) -> Result<Vec<(String, String)>> {
        let default_arch = crate::operations::default_arch()?;
        let archs = std::iter::once(None)
            .chain(crate::operations::compatible_archs()?.into_iter().map(Some))
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for base in self.channel_names() {
            let Ok(mut name) = ChannelName::parse(base) else {
                continue;
            };
            for arch in &archs {
                name.arch = arch.clone();
                let platform = Platform::current(arch.as_deref().unwrap_or(&default_arch))?;
                for file in self.installable_files(&name, platform)? {
                    name.variants = file.sorted_variants();
                    rows.push((name.to_string(), file.label()));
                }
            }
        }
        rows.sort_by(|a, b| numeric_sort::cmp(&a.0, &b.0));
        rows.dedup_by(|a, b| a.0 == b.0);
        Ok(rows)
    }

    pub fn has_channel(&self, channel: &str) -> bool {
        self.channels.contains_key(channel)
    }

    /// The nightly channel names, in a sensible order (`1.10-nightly` before
    /// `1.12-nightly` before `nightly`).
    pub fn channel_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.channels.keys().map(String::as_str).collect();
        names.sort_by(|a, b| numeric_sort::cmp(a, b));
        names
    }

    fn channel(&self, channel: &ChannelName) -> Result<&NightlyChannel> {
        let key = match &channel.base {
            ChannelBase::Nightly { .. } => channel.base.to_string(),
            _ => bail!("'{}' is not a nightly channel.", channel),
        };
        self.channels.get(&key).ok_or_else(|| {
            anyhow!(
                "'{}' is not an available nightly channel. Available nightly channels: {}.",
                key,
                self.channel_names().join(", ")
            )
        })
    }

    /// The installable files of `channel` (standard build and variants) for
    /// `platform`.
    fn installable_files(
        &self,
        channel: &ChannelName,
        platform: Platform,
    ) -> Result<impl Iterator<Item = &NightlyFile>> {
        let entry = self.channel(channel)?;
        Ok(entry
            .files
            .iter()
            .chain(
                entry
                    .variants
                    .iter()
                    .filter(|file| !file.variants.is_empty()),
            )
            .filter(move |file| {
                file.is_for(platform) && file.is_installable() && file.has_valid_variants()
            }))
    }

    /// The build variants of `channel` that exist for `platform`, as the
    /// `+variant` suffixes to append to the channel name (`+opt`, `+nogpl`,
    /// `+assert+opt`, ...), sorted.
    pub fn variant_suffixes(&self, channel: &ChannelName, platform: Platform) -> Vec<String> {
        let mut suffixes: Vec<String> = match self.installable_files(channel, platform) {
            Ok(files) => files
                .filter(|file| !file.variants.is_empty())
                .map(|file| {
                    file.sorted_variants()
                        .iter()
                        .map(|v| format!("+{}", v))
                        .collect()
                })
                .collect(),
            Err(_) => Vec::new(),
        };
        suffixes.sort();
        suffixes.dedup();
        suffixes
    }

    /// The file to install for `channel` on `platform`: the standard build,
    /// or the variant the channel name asks for.
    pub fn select(&self, channel: &ChannelName, platform: Platform) -> Result<&NightlyFile> {
        self.installable_files(channel, platform)?
            .find(|file| file.sorted_variants() == channel.variants)
            .ok_or_else(|| {
                let build = if channel.variants.is_empty() {
                    "build".to_string()
                } else {
                    format!("'{}' build", channel.variant_suffix())
                };
                let available = self.variant_suffixes(channel, platform);
                let hint = if available.is_empty() {
                    String::new()
                } else {
                    format!(" Available variants: {}.", available.join(", "))
                };
                anyhow!(
                    "'{}' has no {} for {}.{}",
                    channel.base,
                    build,
                    platform,
                    hint
                )
            })
    }
}

/// Source, raw catalog and successful-fetch time commit together.
#[derive(Deserialize, Serialize)]
struct Cache {
    source: String,
    checked_at: DateTime<Utc>,
    payload: String,
}

fn cache_source() -> Result<url::Url> {
    crate::utils::get_juliaserver_base_url()?
        .join("bin/nightlies.json")
        .context("Failed to construct nightly catalog URL.")
}

fn decode_cache(bytes: &[u8], source: &str) -> Option<(Cache, NightliesDb)> {
    let cache: Cache = serde_json::from_slice(bytes).ok()?;
    if cache.source != source {
        return None;
    }
    let db = NightliesDb::parse(&cache.payload).ok()?;
    Some((cache, db))
}

pub fn load_nightlies_db(paths: &GlobalPaths) -> Option<NightliesDb> {
    let source = cache_source().ok()?;
    decode_cache(&std::fs::read(&paths.nightliesdb).ok()?, source.as_str()).map(|(_, db)| db)
}

pub fn cache_is_fresh(paths: &GlobalPaths) -> bool {
    let Ok(source) = cache_source() else {
        return false;
    };
    let Ok(bytes) = std::fs::read(&paths.nightliesdb) else {
        return false;
    };
    decode_cache(&bytes, source.as_str()).is_some_and(|(cache, _)| {
        let age = Utc::now().signed_duration_since(cache.checked_at);
        age >= chrono::Duration::zero() && age < chrono::Duration::hours(24)
    })
}

/// Fetch outside the configuration lock and atomically replace a parsed catalog.
pub fn refresh(paths: &GlobalPaths, timeout: Duration) -> Result<NightliesDb> {
    let source = cache_source()?;
    refresh_from(
        paths,
        &source,
        Instant::now() + timeout,
        crate::operations::download_text,
    )
}

fn refresh_from(
    paths: &GlobalPaths,
    source: &url::Url,
    deadline: Instant,
    mut download: impl FnMut(&str, Instant) -> Result<String>,
) -> Result<NightliesDb> {
    let payload = download(source.as_str(), deadline)?;
    let db = NightliesDb::parse(&payload)?;
    let cache = Cache {
        source: source.to_string(),
        checked_at: Utc::now(),
        payload,
    };
    if Instant::now() >= deadline {
        bail!("Nightly database refresh timed out.");
    }
    let parent = paths
        .nightliesdb
        .parent()
        .context("Nightly cache has no parent.")?;
    std::fs::create_dir_all(parent)?;
    let bytes = serde_json::to_vec(&cache)?;
    if Instant::now() >= deadline {
        bail!("Nightly catalog refresh timed out.");
    }
    write_atomically(&paths.nightliesdb, &bytes, deadline)?;
    Ok(db)
}

fn write_atomically(path: &Path, content: &[u8], deadline: Instant) -> Result<()> {
    let parent = path.parent().context("Nightly cache has no parent.")?;
    let temp = tempfile::NamedTempFile::new_in(parent)?;
    std::fs::write(temp.path(), content)?;
    if Instant::now() >= deadline {
        bail!("Nightly catalog refresh timed out.");
    }
    // A transient rename failure can use the old cache. Retrying here could
    // extend a bounded discovery request beyond its deadline.
    temp.persist(path)
        .context("Failed to replace nightly cache.")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "1.13-nightly": {
        "files": [
          {"arch": "x86_64", "os": "linux", "triplet": "x86_64-linux-gnu", "kind": "archive", "extension": "tar.gz",
           "url": "https://julialangnightlies-s3.julialang.org/bin/linux/x86_64/1.13/julia-latest-linux-x86_64.tar.gz",
           "asc-url": "https://julialangnightlies-s3.julialang.org/bin/linux/x86_64/1.13/julia-latest-linux-x86_64.tar.gz.asc"},
          {"arch": "aarch64", "os": "mac", "triplet": "aarch64-apple-darwin14", "kind": "archive", "extension": "dmg",
           "url": "https://julialangnightlies-s3.julialang.org/bin/macos/aarch64/1.13/julia-latest-macos-aarch64.dmg"},
          {"arch": "aarch64", "os": "mac", "triplet": "aarch64-apple-darwin14", "kind": "archive", "extension": "tar.gz",
           "url": "https://julialangnightlies-s3.julialang.org/bin/macos/aarch64/1.13/julia-latest-macos-aarch64.tar.gz"},
          {"arch": "x86_64", "os": "winnt", "triplet": "x86_64-w64-mingw32", "kind": "installer", "extension": "exe",
           "url": "https://julialangnightlies-s3.julialang.org/bin/winnt/x64/1.13/julia-latest-win64.exe"},
          {"arch": "x86_64", "os": "winnt", "triplet": "x86_64-w64-mingw32", "kind": "archive", "extension": "tar.gz",
           "url": "https://julialangnightlies-s3.julialang.org/bin/winnt/x64/1.13/julia-latest-win64.tar.gz"}
        ],
        "variants": [
          {"arch": "aarch64", "os": "mac", "triplet": "aarch64-apple-darwin14", "kind": "archive", "extension": "tar.gz",
           "url": "https://julialang-nogpl.s3.amazonaws.com/bin-nogpl/macosnogpl/aarch64/1.13/julia-latest-macosnogpl-aarch64.tar.gz",
           "variants": ["nogpl"]}
        ]
      },
      "nightly": {
        "files": [
          {"arch": "x86_64", "os": "linux", "triplet": "x86_64-linux-gnu", "kind": "archive", "extension": "tar.gz",
           "url": "https://julialangnightlies-s3.julialang.org/bin/linux/x86_64/julia-latest-linux-x86_64.tar.gz"},
          {"arch": "riscv64", "os": "plan9", "triplet": "riscv64-plan9", "kind": "hologram", "extension": "tar.gz",
           "url": "https://example.invalid/julia-latest-plan9-riscv64.tar.gz", "future-key": true}
        ],
        "variants": [
          {"arch": "x86_64", "os": "linux", "triplet": "x86_64-linux-gnu", "kind": "archive", "extension": "tar.gz",
           "url": "https://julialangnightlies-s3.julialang.org/bin/linuxopt/x86_64/julia-latest-linuxopt-x86_64.tar.gz",
           "variants": ["opt"]},
          {"arch": "x86_64", "os": "linux", "triplet": "x86_64-linux-gnu", "kind": "archive", "extension": "tar.gz",
           "url": "https://julialangnightlies-s3.julialang.org/bin/linuxassert/x86_64/julia-latest-linuxassert-x86_64.tar.gz",
           "variants": ["assert"]},
          {"arch": "x86_64", "os": "linux", "triplet": "x86_64-linux-gnu", "kind": "archive", "extension": "tar.gz",
           "url": "https://example.invalid/julia-latest-linuxoptassert-x86_64.tar.gz",
           "variants": ["opt", "assert"]}
        ],
        "future-key": {}
      },
      "1.10-nightly": {"files": []}
    }"#;

    const LINUX_X64: Platform = Platform {
        os: "linux",
        arch: "x86_64",
    };
    const MAC_ARM: Platform = Platform {
        os: "mac",
        arch: "aarch64",
    };
    const WIN_X64: Platform = Platform {
        os: "winnt",
        arch: "x86_64",
    };

    fn db() -> NightliesDb {
        NightliesDb::parse(FIXTURE).unwrap()
    }

    fn name(channel: &str) -> ChannelName {
        ChannelName::parse(channel).unwrap()
    }

    #[test]
    fn parses_leniently() {
        let db = db();
        assert_eq!(
            db.channel_names(),
            ["1.10-nightly", "1.13-nightly", "nightly"]
        );
        assert!(db.has_channel("nightly"));
        assert!(!db.has_channel("1.9-nightly"));
    }

    #[test]
    fn selects_the_tarball_for_the_platform() {
        let db = db();
        let file = db.select(&name("1.13-nightly"), LINUX_X64).unwrap();
        assert!(file.url.ends_with("/1.13/julia-latest-linux-x86_64.tar.gz"));
        assert_eq!(file.label(), "latest-linux-x86_64");

        // The arch suffix of the channel name does not matter here: the
        // caller maps it to the platform.
        let file = db.select(&name("nightly~x86"), LINUX_X64).unwrap();
        assert!(file.url.ends_with("/julia-latest-linux-x86_64.tar.gz"));

        // Tarballs win over DMGs and installers.
        let file = db.select(&name("1.13-nightly"), MAC_ARM).unwrap();
        assert_eq!(file.extension, "tar.gz");
        let file = db.select(&name("1.13-nightly"), WIN_X64).unwrap();
        assert_eq!(file.label(), "latest-win64");
    }

    #[test]
    fn ignores_unsupported_abis() {
        let mut db = db();
        let entry = db.channels.get_mut("nightly").unwrap();
        let mut musl = entry.files[0].clone();
        musl.triplet = "x86_64-linux-musl".to_string();
        musl.url = "https://example.invalid/musl.tar.gz".to_string();
        entry.files.insert(0, musl);
        assert_eq!(
            db.select(&name("nightly"), LINUX_X64).unwrap().triplet,
            "x86_64-linux-gnu"
        );
        db.channels.get_mut("nightly").unwrap().files.remove(1);
        assert!(db.select(&name("nightly"), LINUX_X64).is_err());
    }

    #[test]
    fn selects_variants() {
        let db = db();
        let file = db.select(&name("nightly+opt"), LINUX_X64).unwrap();
        assert_eq!(file.label(), "latest-linuxopt-x86_64");
        let file = db.select(&name("nightly+assert+opt"), LINUX_X64).unwrap();
        assert_eq!(file.label(), "latest-linuxoptassert-x86_64");
        let file = db
            .select(&name("1.13-nightly+nogpl~aarch64"), MAC_ARM)
            .unwrap();
        assert_eq!(file.label(), "latest-macosnogpl-aarch64");

        // The standard build never resolves to a variant, and vice versa.
        assert!(db
            .select(&name("nightly"), LINUX_X64)
            .unwrap()
            .variants
            .is_empty());
        let err = db.select(&name("nightly+nogpl"), LINUX_X64).unwrap_err();
        assert_eq!(
            err.to_string(),
            "'nightly' has no '+nogpl' build for linux/x86_64. Available variants: +assert, +assert+opt, +opt."
        );
        let err = db.select(&name("1.13-nightly+opt"), WIN_X64).unwrap_err();
        assert_eq!(
            err.to_string(),
            "'1.13-nightly' has no '+opt' build for winnt/x86_64."
        );

        assert_eq!(
            db.variant_suffixes(&name("nightly"), LINUX_X64),
            ["+assert", "+assert+opt", "+opt"]
        );
        assert!(db.variant_suffixes(&name("nightly"), MAC_ARM).is_empty());
        assert!(db
            .variant_suffixes(&name("1.9-nightly"), MAC_ARM)
            .is_empty());
    }

    #[test]
    fn reports_missing_channels_and_builds() {
        let db = db();
        let err = db.select(&name("1.9-nightly"), LINUX_X64).unwrap_err();
        assert!(err
            .to_string()
            .contains("Available nightly channels: 1.10-nightly, 1.13-nightly, nightly"));

        let err = db.select(&name("1.10-nightly"), LINUX_X64).unwrap_err();
        assert_eq!(
            err.to_string(),
            "'1.10-nightly' has no build for linux/x86_64."
        );

        assert!(db.select(&name("release"), LINUX_X64).is_err());
        assert!(db.select(&name("pr123"), LINUX_X64).is_err());
    }

    #[test]
    fn unnamed_variants_are_not_standard_builds() {
        for variants in ["", r#", "variants": []"#] {
            let json = format!(
                r#"{{"nightly": {{"files": [], "variants": [{{
                    "os": "linux", "arch": "x86_64", "triplet": "x86_64-linux-gnu",
                    "kind": "archive", "extension": "tar.gz",
                    "url": "https://example.org/variant.tar.gz"{variants}
                }}]}}}}"#
            );
            let db = NightliesDb::parse(&json).unwrap();
            assert!(db.select(&name("nightly"), LINUX_X64).is_err());
        }
    }

    #[test]
    fn current_platform_maps_juliaup_arch_ids() {
        let platform = Platform::current("x64").unwrap();
        assert_eq!(platform.arch, "x86_64");
        assert_eq!(Platform::current("x86").unwrap().arch, "i686");
        assert_eq!(Platform::current("aarch64").unwrap().arch, "aarch64");
        assert!(Platform::current("mips").is_err());
    }

    fn paths(dir: &Path) -> GlobalPaths {
        GlobalPaths {
            juliauphome: dir.to_path_buf(),
            juliaupconfig: dir.join("config.json"),
            lockfile: dir.join("config.lock"),
            versiondb: dir.join("versions.json"),
            nightliesdb: dir.join("nightly-cache.json"),
            #[cfg(feature = "selfupdate")]
            juliaupselfhome: dir.to_path_buf(),
            #[cfg(feature = "selfupdate")]
            juliaupselfconfig: dir.join("self.json"),
            #[cfg(feature = "selfupdate")]
            juliaupselfbin: dir.to_path_buf(),
        }
    }

    #[test]
    fn spelling_and_bad_entries() {
        let db = db();
        let selected = db.select(&name("nightly+opt+assert"), LINUX_X64).unwrap();
        assert_eq!(selected.variants, ["opt", "assert"]);
        assert!(std::ptr::eq(
            selected,
            db.select(&name("nightly+assert+opt+opt"), LINUX_X64)
                .unwrap()
        ));
        let mut json: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        json["nightly+"] = json["nightly"].clone();
        json["nightly"]["variants"][0]["variants"] = serde_json::json!(["opt", "opt"]);
        json["nightly"]["variants"][1]["variants"] = serde_json::json!(["bad+"]);
        let db = NightliesDb::parse(&json.to_string()).unwrap();
        assert!(!db.has_channel("nightly+"));
        assert_eq!(
            db.variant_suffixes(&name("nightly"), LINUX_X64),
            ["+assert+opt"]
        );
        assert!(db.select(&name("nightly"), LINUX_X64).is_ok());
    }

    #[test]
    fn refresh_preserves_cache_on_failure_and_scopes_source() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        let source = url::Url::parse("https://example.org/bin/nightlies.json").unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        refresh_from(&paths, &source, deadline, |url, _| {
            assert_eq!(url, source.as_str());
            Ok(FIXTURE.into())
        })
        .unwrap();
        let saved = std::fs::read(&paths.nightliesdb).unwrap();
        assert!(decode_cache(&saved, source.as_str()).is_some());
        assert!(decode_cache(&saved, "https://other.example/bin/nightlies.json").is_none());
        for response in [Err(anyhow!("offline")), Ok("not json".into())] {
            let mut response = Some(response);
            assert!(
                refresh_from(&paths, &source, deadline, |_, _| response.take().unwrap()).is_err()
            );
            assert_eq!(std::fs::read(&paths.nightliesdb).unwrap(), saved);
        }
        assert!(decode_cache(b"corrupt cache", source.as_str()).is_none());
    }

    #[test]
    fn expired_refresh_never_commits() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        let source = url::Url::parse("https://example.org/bin/nightlies.json").unwrap();
        assert!(refresh_from(
            &paths,
            &source,
            Instant::now() - Duration::from_secs(1),
            |_, _| Ok(FIXTURE.into())
        )
        .is_err());
        assert!(!paths.nightliesdb.exists());
    }

    #[test]
    fn expired_cache_write_preserves_previous_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        std::fs::write(&path, b"previous catalog").unwrap();
        assert!(write_atomically(
            &path,
            b"new catalog",
            Instant::now() - Duration::from_secs(1),
        )
        .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"previous catalog");
    }
}
