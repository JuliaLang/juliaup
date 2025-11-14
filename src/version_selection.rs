use anyhow::{anyhow, Context, Result};
use semver::Version;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use toml::Value;

use crate::jsonstructs_versionsdb::JuliaupVersionDB;
use crate::utils::{print_juliaup_style, JuliaupMessageType};

// Constants matching Julia's base/loading.jl
// https://github.com/JuliaLang/julia/blob/dd80509227adbd525737244ebabc95ec5d634354/base/loading.jl#L625C1-L631C2
const PROJECT_NAMES: &[&str] = &["JuliaProject.toml", "Project.toml"];
// excludes versioned manifests here
const MANIFEST_NAMES: &[&str] = &["JuliaManifest.toml", "Manifest.toml"];

#[cfg(windows)]
pub const LOAD_PATH_SEPARATOR: &str = ";";
#[cfg(not(windows))]
pub const LOAD_PATH_SEPARATOR: &str = ":";

fn find_named_file(dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|file| dir.join(file))
        .find(|path| path.is_file())
}

fn find_project_file_in_dir(dir: &Path) -> Option<PathBuf> {
    find_named_file(dir, PROJECT_NAMES)
}

fn resolve_depot_paths(depot_path: Option<&OsStr>) -> Result<Vec<PathBuf>> {
    if let Some(paths) = depot_path {
        let candidates: Vec<_> = std::env::split_paths(paths).collect();
        if !candidates.is_empty() {
            return Ok(candidates);
        }
    }

    let home = dirs::home_dir()
        .ok_or_else(|| anyhow!("Could not determine the path of the user home directory."))?;
    Ok(vec![home.join(".julia")])
}

fn find_named_environment(depot_paths: &[PathBuf], env_name: &str) -> Option<PathBuf> {
    depot_paths.iter().find_map(|depot| {
        let env_dir = depot.join("environments").join(env_name);
        if env_dir.is_dir() {
            find_project_file_in_dir(&env_dir)
        } else {
            None
        }
    })
}

fn default_named_environment_path(depot_paths: &[PathBuf], env_name: &str) -> Option<PathBuf> {
    depot_paths.first().map(|depot| {
        depot
            .join("environments")
            .join(env_name)
            .join(PROJECT_NAMES.last().copied().unwrap_or("Project.toml"))
    })
}

fn should_skip_load_path_entry(entry: &str) -> bool {
    entry.is_empty() || entry == "@" || entry == "@stdlib" || entry.starts_with("@v")
}

/// Search upward from dir for a project file (Julia's current_project)
/// https://github.com/JuliaLang/julia/blob/dd80509227adbd525737244ebabc95ec5d634354/base/initdefs.jl#L203-L216
pub fn current_project(dir: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir();
    let mut current = dir;

    loop {
        if let Some(project) = find_project_file_in_dir(current) {
            return Some(project);
        }

        // Bail at home directory
        if let Some(ref home_dir) = home {
            if current == home_dir.as_path() {
                break;
            }
        }

        // Move to parent
        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => break,
        }
    }

    None
}

/// Load path expansion (Julia's load_path_expand)
/// Turn LOAD_PATH entries into concrete paths
/// https://github.com/JuliaLang/julia/blob/dd80509227adbd525737244ebabc95ec5d634354/base/initdefs.jl#L277-L323
pub fn load_path_expand_impl(
    env: &str,
    current_dir: &Path,
    depot_path: Option<&std::ffi::OsStr>,
) -> Result<Option<PathBuf>> {
    // Named environment?
    if let Some(stripped) = env.strip_prefix('@') {
        match stripped {
            "" => return Ok(None),
            "." => return Ok(current_project(current_dir)),
            "stdlib" => return Ok(None),
            _ => {}
        }

        // Named environment like "@v1.10"
        let depot_paths = resolve_depot_paths(depot_path)?;

        if let Some(project) = find_named_environment(&depot_paths, stripped) {
            return Ok(Some(project));
        }

        return Ok(default_named_environment_path(&depot_paths, stripped));
    }

    // Otherwise, it's a path
    let mut path = PathBuf::from(shellexpand::tilde(env).as_ref());
    if path.is_relative() {
        path = current_dir.join(path);
    }

    if path.is_dir() {
        // Directory with a project file?
        if let Some(project_file) = find_project_file_in_dir(&path) {
            return Ok(Some(project_file));
        }
    }

    // Package dir or path to project file
    Ok(Some(path))
}

/// Search JULIA_LOAD_PATH for the first valid project with a manifest
pub fn find_project_from_load_path(
    load_path: &str,
    current_dir: &Path,
    depot_path: Option<&std::ffi::OsStr>,
) -> Result<Option<PathBuf>> {
    for entry in load_path.split(LOAD_PATH_SEPARATOR).map(str::trim) {
        if should_skip_load_path_entry(entry) {
            continue;
        }

        let Ok(Some(project_file)) = load_path_expand_impl(entry, current_dir, depot_path) else {
            continue;
        };

        if project_file_manifest_path(&project_file).is_some() {
            log::debug!(
                "VersionDetect::Found valid project in JULIA_LOAD_PATH entry: {}",
                entry
            );
            return Ok(Some(project_file));
        }
    }

    log::debug!("VersionDetect::No valid project with manifest found in JULIA_LOAD_PATH");
    Ok(None)
}

/// Check if a Julia option requires an argument (mimics getopt's required_argument)
/// Based on jloptions.c:421-493
pub fn julia_option_requires_arg(opt: &str) -> bool {
    // TODO: Rewrite with clap.rs in future
    // Options with = already include their argument
    if opt.contains('=') {
        return false;
    }

    // Handle short options: from shortopts = "+vhqH:e:E:L:J:C:it:p:O:g:m:"
    // Options WITHOUT ':' are no_argument, 'O' and 'g' are optional_argument

    // Check if this is a short option (e.g., "-e" but not "--eval")
    let short_option = opt
        .strip_prefix('-')
        .filter(|s| !s.starts_with('-') && s.len() == 1)
        .and_then(|s| s.chars().next());

    if let Some(short) = short_option {
        // no_argument: v, h, q, i
        // optional_argument: O, g
        return !matches!(short, 'v' | 'h' | 'q' | 'i' | 'O' | 'g');
    }

    // Long options: list no_argument and optional_argument (everything else requires an argument)
    !matches!(
        opt,
        // no_argument options
        "--version" | "--help" | "--help-hidden" | "--interactive" | "--quiet"
            | "--experimental" | "--lisp" | "--image-codegen" | "--rr-detach"
            | "--strip-metadata" | "--strip-ir" | "--gc-sweep-always-full"
            | "--trace-compile-timing"
            // optional_argument options
            | "--project" | "--code-coverage" | "--track-allocation" | "--optimize"
            | "--min-optlevel" | "--debug-info" | "--worker" | "--trim" | "--trace-eval"
    )
}

/// Initialize the active project (Julia's init_active_project)
/// Returns the project file path based on --project flag or JULIA_PROJECT env
/// https://github.com/JuliaLang/julia/blob/dd80509227adbd525737244ebabc95ec5d634354/base/initdefs.jl#L263-L272
pub fn init_active_project_impl(
    args: &[String],
    current_dir: &Path,
    julia_project: Option<&str>,
    depot_path: Option<&std::ffi::OsStr>,
) -> Result<Option<PathBuf>> {
    // Check for --project flag in args
    // Stop parsing at "--" or the first positional argument (non-flag)
    // to match Julia's argument parsing behavior
    let mut project_cli = None;
    let mut args_iter = args.iter().skip(1);
    while let Some(arg) = args_iter.next() {
        // Stop at -- separator (everything after is for the script)
        if arg == "--" {
            break;
        }

        // Stop at first positional argument (doesn't start with -)
        if !arg.starts_with('-') {
            break;
        }

        if ["--project", "--projec", "--proje", "--proj"].contains(&arg.as_str()) {
            project_cli = Some(None);
        } else if let Some(value) = ["--project=", "--projec=", "--proje=", "--proj="]
            .iter()
            .find_map(|prefix| arg.strip_prefix(prefix))
        {
            project_cli = Some(Some(value.to_string()));
        } else if julia_option_requires_arg(arg) {
            // This option consumes the next token as its argument
            // Skip it to avoid treating the argument as a flag
            args_iter.next();
        }
    }

    // Determine project spec
    let project = if let Some(spec) = project_cli {
        // --project flag takes precedence
        // If --project has no value, treat as "@." (search upward)
        Some(spec.unwrap_or_else(|| "@.".to_string()))
    } else {
        // Check JULIA_PROJECT env
        julia_project.map(|v| {
            if v.trim().is_empty() {
                "@.".to_string() // Empty JULIA_PROJECT means "@."
            } else {
                v.to_string()
            }
        })
    };

    let project = match project {
        None => return Ok(None),
        Some(p) => p,
    };

    // Expand the project spec using load_path_expand
    load_path_expand_impl(&project, current_dir, depot_path)
}

/// Find project file's corresponding manifest file (Julia's project_file_manifest_path)
/// https://github.com/JuliaLang/julia/blob/dd80509227adbd525737244ebabc95ec5d634354/base/loading.jl#L892-L928
///
/// Julia's priority order (from manifest_names constant):
/// 1. JuliaManifest-v{major}.{minor}.toml
/// 2. Manifest-v{major}.{minor}.toml
/// 3. JuliaManifest.toml
/// 4. Manifest.toml
///
/// Since we don't know the Julia version yet (that's what we're determining),
/// we find the highest versioned manifest.
pub fn project_file_manifest_path(project_file: &Path) -> Option<PathBuf> {
    let dir = project_file.parent()?;

    if !project_file.exists() || !project_file.is_file() {
        return None;
    }

    // Parse project file to check for explicit manifest field
    let project_content = fs::read_to_string(project_file).ok()?;
    let parsed_project: Value = toml::from_str(&project_content).ok()?;

    // Check for explicit manifest field
    if let Some(Value::String(explicit_manifest)) = parsed_project.get("manifest") {
        let manifest_file = if Path::new(explicit_manifest).is_absolute() {
            PathBuf::from(explicit_manifest)
        } else {
            dir.join(explicit_manifest)
        };
        if manifest_file.exists() && manifest_file.is_file() {
            return Some(manifest_file);
        }
    }

    // Check for versioned manifests first (highest priority after explicit)
    // Look for both JuliaManifest-v*.toml and Manifest-v*.toml
    if let Some(versioned_manifest) = find_highest_versioned_manifest(dir) {
        return Some(versioned_manifest);
    }

    find_named_file(dir, MANIFEST_NAMES)
}

/// Determines the Julia version requirement from a project's Manifest.toml.
/// based on arguments to julia and env variables
///
/// Project can be specified via (in priority order):
/// - `--project=path` → uses specified path (file or directory)
/// - `--project=@name` → uses depot environment (e.g., @v1.10 looks in ~/.julia/environments/v1.10)
/// - `--project` (no value) → searches upward from current directory for Project.toml
/// - `JULIA_PROJECT=path` → uses specified path
/// - `JULIA_PROJECT=@name` → uses depot environment (e.g., @v1.10)
/// - `JULIA_PROJECT=""` (empty) → searches upward from current directory (@.)
/// - `JULIA_LOAD_PATH` → searches entries in load path for first valid project
///
/// Returns the version string from Manifest.toml's `julia_version`.
/// Returns `None` if no manifest is found (falls back to release channel).
/// Returns error only if project was specified but couldn't be resolved.
pub fn determine_project_version_spec(args: &[String]) -> Result<Option<String>> {
    determine_project_version_spec_impl(
        args,
        std::env::var("JULIA_PROJECT").ok(),
        std::env::var("JULIA_LOAD_PATH").ok(),
        &std::env::current_dir().with_context(|| "Failed to determine current directory.")?,
    )
}

pub fn determine_project_version_spec_impl(
    args: &[String],
    julia_project: Option<String>,
    julia_load_path: Option<String>,
    current_dir: &Path,
) -> Result<Option<String>> {
    let depot_path = std::env::var_os("JULIA_DEPOT_PATH");

    // Try --project or JULIA_PROJECT first
    if let Some(project_file) = init_active_project_impl(
        args,
        current_dir,
        julia_project.as_deref(),
        depot_path.as_deref(),
    )? {
        return extract_version_from_project(project_file);
    }

    // Fall back to JULIA_LOAD_PATH
    if let Some(ref load_path) = julia_load_path {
        if let Some(project_file) =
            find_project_from_load_path(load_path, current_dir, depot_path.as_deref())?
        {
            return extract_version_from_project(project_file);
        }
    }

    // No project found
    log::debug!("VersionDetect::No project specification found");
    Ok(None)
}

pub fn extract_version_from_project(project_file: PathBuf) -> Result<Option<String>> {
    log::debug!(
        "VersionDetect::Using project file: {}",
        project_file.display()
    );

    // Find the manifest file using Julia's project_file_manifest_path logic
    let manifest_path = match project_file_manifest_path(&project_file) {
        Some(path) => path,
        None => {
            log::debug!("VersionDetect::No manifest file found for project");
            return Ok(None);
        }
    };

    log::debug!(
        "VersionDetect::Detected manifest file: {}",
        manifest_path.display()
    );

    // Read julia_version from manifest
    if let Some(version) = read_manifest_julia_version(&manifest_path)? {
        log::debug!(
            "VersionDetect::Read Julia version from manifest: {}",
            version
        );
        return Ok(Some(version));
    }

    log::debug!("VersionDetect::Manifest file exists but does not contain julia_version field");
    Ok(None)
}

/// Find highest versioned manifest, preferring JuliaManifest over Manifest for same version
pub fn find_highest_versioned_manifest(project_root: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(project_root).ok()?;

    // Track highest version for both JuliaManifest and Manifest separately
    let mut highest_julia: Option<(Version, PathBuf)> = None;
    let mut highest_manifest: Option<(Version, PathBuf)> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            // Check for versioned manifests: JuliaManifest-v*.toml or Manifest-v*.toml
            // Skip files that don't match either pattern
            if !filename.starts_with("JuliaManifest-v") && !filename.starts_with("Manifest-v") {
                continue;
            }

            let (prefix, target) = if filename.starts_with("JuliaManifest-v") {
                ("JuliaManifest-v", &mut highest_julia)
            } else {
                ("Manifest-v", &mut highest_manifest)
            };

            if let Some(version) = filename
                .strip_prefix(prefix)
                .and_then(|s| s.strip_suffix(".toml"))
                .and_then(parse_version_lenient)
            {
                // Update highest if this version is greater or if none exists yet
                let should_update = match target {
                    Some((current_version, _)) => version > *current_version,
                    None => true,
                };
                if should_update {
                    *target = Some((version, path));
                }
            }
        }
    }

    // Return highest version, preferring JuliaManifest for same version
    match (highest_julia, highest_manifest) {
        (Some((jv, jpath)), Some((mv, mpath))) => Some(if jv >= mv { jpath } else { mpath }),
        (Some((_, jpath)), None) => Some(jpath),
        (None, Some((_, mpath))) => Some(mpath),
        (None, None) => None,
    }
}

// Parse a version string leniently, handling incomplete versions like "1.11" or "1"
pub fn parse_version_lenient(version_str: &str) -> Option<Version> {
    // First try parsing as-is
    if let Ok(version) = Version::parse(version_str) {
        return Some(version);
    }

    // If that fails, try adding missing components
    let parts: Vec<&str> = version_str.split('.').collect();
    let normalized = match parts.len() {
        1 => format!("{}.0.0", parts[0]),
        2 => format!("{}.{}.0", parts[0], parts[1]),
        _ => return None,
    };

    Version::parse(&normalized).ok()
}

pub fn read_manifest_julia_version(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        log::debug!(
            "VersionDetect::Manifest file `{}` not found while attempting to resolve Julia version.",
            path.display()
        );
        return Ok(None);
    }

    let manifest_content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read manifest file `{}`.", path.display()))?;

    let manifest: Value = toml::from_str(&manifest_content).with_context(|| {
        format!(
            "Failed to parse manifest file `{}` as TOML.",
            path.display()
        )
    })?;

    Ok(manifest
        .get("julia_version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string()))
}

pub fn parse_db_version(version: &str) -> Result<Version> {
    let base = version
        .split('+')
        .next()
        .ok_or_else(|| anyhow!("Invalid version string `{}`.", version))?;
    Version::parse(base).with_context(|| format!("Failed to parse version `{}`.", base))
}

fn versioned_nightly_channel(major: u64, minor: u64) -> String {
    format!("{}.{}-nightly", major, minor)
}

impl JuliaupVersionDB {
    pub fn max_available_version(&self) -> Option<Version> {
        self.available_versions
            .keys()
            .filter_map(|key| parse_db_version(key).ok())
            .max()
    }

    /// Find the maximum version (including prerelease) for a given major.minor series
    pub fn max_version_for_minor(&self, major: u64, minor: u64) -> Option<Version> {
        self.available_channels
            .keys()
            .filter_map(|key| parse_db_version(key).ok())
            .filter(|version| version.major == major && version.minor == minor)
            .max()
    }
}

pub fn resolve_auto_channel(required: String, versions_db: &JuliaupVersionDB) -> Result<String> {
    // Check if exact version is available
    if versions_db.available_channels.contains_key(&required) {
        return Ok(required);
    }

    // Parse the required version
    let required_version = Version::parse(&required).with_context(|| {
        format!(
            "Failed to parse Julia version `{}` from manifest.",
            required
        )
    })?;

    // Handle prerelease versions (e.g., 1.12.1-DEV, 1.13.0-rc1)
    // Prereleases should use nightly channels because they represent development/testing versions
    if !required_version.pre.is_empty() {
        // Check if a version-specific nightly channel exists (e.g., 1.12-nightly)
        let versioned_nightly =
            versioned_nightly_channel(required_version.major, required_version.minor);

        if versions_db
            .available_channels
            .contains_key(&versioned_nightly)
        {
            print_juliaup_style(
                "Info",
                &format!(
                    "Manifest specifies prerelease Julia {}. Using {} channel.",
                    required, versioned_nightly
                ),
                JuliaupMessageType::Progress,
            );
            return Ok(versioned_nightly);
        }

        // Fall back to main nightly channel
        print_juliaup_style(
            "Info",
            &format!(
                "Manifest specifies prerelease Julia {}. Using nightly channel.",
                required
            ),
            JuliaupMessageType::Progress,
        );
        return Ok("nightly".to_string());
    }

    // Check if the requested version is higher than any known version for this minor series
    // This handles both regular versions (e.g., 1.12.55 > 1.12.1) and prerelease versions
    // (e.g., 1.12.0-rc1 when only 1.11.x exists)
    let max_version_for_minor =
        versions_db.max_version_for_minor(required_version.major, required_version.minor);

    if let Some(max_minor_version) = &max_version_for_minor {
        if &required_version > max_minor_version {
            // The requested version is higher than any known version for this minor series
            let channel = versioned_nightly_channel(required_version.major, required_version.minor);
            print_juliaup_style(
                "Info",
                &format!(
                    "Manifest specifies Julia {} but the highest known version for {}.{} is {}. Using {} channel.",
                    required,
                    required_version.major,
                    required_version.minor,
                    max_minor_version,
                    channel
                ),
                JuliaupMessageType::Progress,
            );
            return Ok(channel);
        }
    }

    // Check if requested version is higher than any known version overall
    // This handles the case where we have 1.12.x but request 1.13.0
    let max_known_version = versions_db.max_available_version();
    if let Some(max_version) = &max_known_version {
        if &required_version > max_version {
            // Check if a version-specific nightly channel exists (e.g., 1.13-nightly)
            let versioned_nightly =
                versioned_nightly_channel(required_version.major, required_version.minor);

            if versions_db
                .available_channels
                .contains_key(&versioned_nightly)
            {
                print_juliaup_style(
                    "Info",
                    &format!(
                        "Manifest specifies Julia {} but the highest known version is {}. Using {} channel.",
                        required, max_version, versioned_nightly
                    ),
                    JuliaupMessageType::Progress,
                );
                return Ok(versioned_nightly);
            }

            // Fall back to main nightly channel
            print_juliaup_style(
                "Info",
                &format!(
                    "Manifest specifies Julia {} but the highest known version is {}. Using nightly channel.",
                    required, max_version
                ),
                JuliaupMessageType::Progress,
            );
            return Ok("nightly".to_string());
        }
    } else {
        // No versions in database at all, use nightly
        print_juliaup_style(
            "Info",
            &format!(
                "Manifest specifies Julia {} but no versions are known. Using nightly channel.",
                required
            ),
            JuliaupMessageType::Progress,
        );
        return Ok("nightly".to_string());
    }

    Err(anyhow!(
        "Julia version `{}` requested by Project.toml/Manifest.toml is not available in the versions database.",
        required
    ))
}

pub fn get_auto_channel(
    args: &[String],
    versions_db: &JuliaupVersionDB,
    manifest_version_detect: bool,
) -> Result<Option<String>> {
    if !manifest_version_detect {
        Ok(None)
    } else if let Some(required_version) = determine_project_version_spec(args)? {
        resolve_auto_channel(required_version, versions_db).map(Some)
    } else {
        Ok(None)
    }
}
