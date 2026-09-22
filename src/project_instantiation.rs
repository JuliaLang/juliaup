//! Fast checks whether the packages recorded in a manifest are installed, so
//! that the launcher only starts Julia to run `Pkg.instantiate` when needed.

use anyhow::{anyhow, bail, Context, Result};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use toml::{Table, Value};

/// CRC-32C (Castagnoli), continuing from `crc` like Julia's `_crc32c`.
fn crc32c(crc: u32, data: &[u8]) -> u32 {
    const POLY: u32 = 0x82F6_3B78;
    let mut crc = !crc;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ POLY
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

const SLUG_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

fn slug(mut value: u32, len: usize) -> String {
    let n = SLUG_CHARS.len() as u32;
    (0..len)
        .map(|_| {
            let digit = value % n;
            value /= n;
            SLUG_CHARS[digit as usize] as char
        })
        .collect()
}

fn parse_uuid(uuid: &str) -> Result<u128> {
    let hex: String = uuid.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        bail!("Invalid UUID `{}`.", uuid);
    }
    u128::from_str_radix(&hex, 16).with_context(|| format!("Invalid UUID `{}`.", uuid))
}

fn parse_sha1(sha1: &str) -> Result<[u8; 20]> {
    let invalid = || anyhow!("Invalid git-tree-sha1 `{}`.", sha1);
    if sha1.len() != 40 || !sha1.is_ascii() {
        return Err(invalid());
    }
    let mut bytes = [0u8; 20];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&sha1[2 * i..2 * i + 2], 16).map_err(|_| invalid())?;
    }
    Ok(bytes)
}

/// The directory name a package version is installed under in
/// `<depot>/packages/<name>/` (Julia's `Base.version_slug`).
pub fn version_slug(uuid: &str, tree_sha1: &str, len: usize) -> Result<String> {
    let uuid = parse_uuid(uuid)?;
    let sha1 = parse_sha1(tree_sha1)?;
    // Julia hashes the in-memory (little-endian) representation of the UInt128
    let crc = crc32c(0, &uuid.to_le_bytes());
    let crc = crc32c(crc, &sha1);
    Ok(slug(crc, len))
}

/// The depots Julia searches for installed packages (Julia's `DEPOT_PATH`).
///
/// `julia_bin` is the Julia binary that will be launched; its bundled depots
/// are part of the default depot path.
pub fn depot_paths(julia_depot_path: Option<&OsStr>, julia_bin: Option<&Path>) -> Vec<PathBuf> {
    let mut defaults = Vec::new();
    if let Some(home) = dirs::home_dir() {
        defaults.push(home.join(".julia"));
    }
    if let Some(share) = julia_bin
        .and_then(|bin| bin.parent())
        .and_then(|bindir| bindir.parent())
    {
        defaults.push(share.join("local").join("share").join("julia"));
        defaults.push(share.join("share").join("julia"));
    }

    let Some(value) = julia_depot_path.filter(|v| !v.is_empty()) else {
        return defaults;
    };

    let mut depots = Vec::new();
    for entry in std::env::split_paths(value) {
        if entry.as_os_str().is_empty() {
            // An empty entry expands to the default depots
            depots.extend(defaults.iter().cloned());
        } else {
            depots.push(entry);
        }
    }
    depots
}

/// Why a project needs to be instantiated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstantiationNeed {
    /// The project has dependencies but no manifest.
    NoManifest,
    /// These packages from the manifest are not installed.
    MissingPackages(Vec<String>),
}

/// The package entries of a manifest, as (name, entry) pairs. Supports both
/// manifest format 2.0 (`[[deps.Name]]`) and the older format 1.0 (`[[Name]]`).
fn manifest_entries(manifest: &Table) -> Vec<(&str, &Table)> {
    let deps: Box<dyn Iterator<Item = (&String, &Value)>> =
        if manifest.contains_key("manifest_format") {
            match manifest.get("deps").and_then(|d| d.as_table()) {
                Some(deps) => Box::new(deps.iter()),
                None => Box::new(std::iter::empty()),
            }
        } else {
            Box::new(manifest.iter())
        };

    deps.filter_map(|(name, value)| value.as_array().map(|entries| (name, entries)))
        .flat_map(|(name, entries)| {
            entries
                .iter()
                .filter_map(|entry| entry.as_table())
                .map(move |entry| (name.as_str(), entry))
        })
        .collect()
}

fn is_package_installed(
    name: &str,
    entry: &Table,
    manifest_dir: &Path,
    depots: &[PathBuf],
) -> Result<bool> {
    if let Some(path) = entry.get("path").and_then(|p| p.as_str()) {
        // Developed package tracked by path
        return Ok(manifest_dir.join(path).is_dir());
    }

    let Some(tree_sha1) = entry.get("git-tree-sha1").and_then(|s| s.as_str()) else {
        // Standard library shipped with Julia
        return Ok(true);
    };
    let uuid = entry
        .get("uuid")
        .and_then(|u| u.as_str())
        .ok_or_else(|| anyhow!("The manifest entry for `{}` has no uuid.", name))?;

    // Mirrors Pkg.Operations.find_installed: slugs of length 5, and 4 (the old default)
    for len in [5, 4] {
        let slug = version_slug(uuid, tree_sha1, len)?;
        if depots
            .iter()
            .any(|depot| depot.join("packages").join(name).join(&slug).is_dir())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Check whether a project needs to be instantiated, i.e. whether any package
/// recorded in `manifest_file` is not installed in `depots`.
///
/// Artifacts and precompilation are not checked; `Pkg.instantiate` takes care
/// of them when it runs.
pub fn check_instantiation(
    manifest_file: Option<&Path>,
    has_deps: bool,
    depots: &[PathBuf],
) -> Result<Option<InstantiationNeed>> {
    let Some(manifest_file) = manifest_file else {
        return Ok(has_deps.then_some(InstantiationNeed::NoManifest));
    };

    let content = fs::read_to_string(manifest_file)
        .with_context(|| format!("Failed to read `{}`", manifest_file.display()))?;
    let manifest: Table = toml::from_str(&content)
        .with_context(|| format!("Failed to parse `{}` as TOML", manifest_file.display()))?;
    let manifest_dir = manifest_file.parent().unwrap_or(Path::new("."));

    let mut missing = Vec::new();
    for (name, entry) in manifest_entries(&manifest) {
        let installed = is_package_installed(name, entry, manifest_dir, depots)
            .with_context(|| format!("Failed to check `{}`", manifest_file.display()))?;
        if !installed {
            missing.push(name.to_string());
        }
    }

    Ok((!missing.is_empty()).then_some(InstantiationNeed::MissingPackages(missing)))
}
