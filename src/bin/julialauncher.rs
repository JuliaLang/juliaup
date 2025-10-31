use anyhow::{anyhow, bail, Context, Result};
use console::{style, Term};
use dialoguer::Select;
use expand_tilde::expand_tilde;
use is_terminal::IsTerminal;
use itertools::Itertools;
use juliaup::config_file::{
    load_config_db, load_mut_config_db, save_config_db, JuliaupConfig, JuliaupConfigChannel,
};
use juliaup::global_paths::get_paths;
use juliaup::jsonstructs_versionsdb::JuliaupVersionDB;
use juliaup::operations::{is_pr_channel, is_valid_channel};
use juliaup::versions_file::load_versions_db;
#[cfg(not(windows))]
use nix::{
    sys::wait::{waitpid, WaitStatus},
    unistd::{fork, ForkResult},
};
use normpath::PathExt;
use semver::Version;
use std::fs;
#[cfg(not(windows))]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, RawHandle};
use std::path::Path;
use std::path::PathBuf;
use toml::Value;
#[cfg(windows)]
use windows::Win32::System::{
    JobObjects::{AssignProcessToJobObject, SetInformationJobObject},
    Threading::GetCurrentProcess,
};

#[derive(thiserror::Error, Debug)]
#[error("{msg}")]
pub struct UserError {
    msg: String,
}

fn get_juliaup_path() -> Result<PathBuf> {
    let my_own_path = std::env::current_exe()
        .with_context(|| "std::env::current_exe() did not find its own path.")?
        .canonicalize()
        .with_context(|| "Failed to canonicalize the path to the Julia launcher.")?;

    let juliaup_path = my_own_path
        .parent()
        .unwrap() // unwrap OK here because this can't happen
        .join(format!("juliaup{}", std::env::consts::EXE_SUFFIX));

    Ok(juliaup_path)
}

fn do_initial_setup(juliaupconfig_path: &Path) -> Result<()> {
    if !juliaupconfig_path.exists() {
        let juliaup_path = get_juliaup_path().with_context(|| "Failed to obtain juliaup path.")?;

        std::process::Command::new(juliaup_path)
            .arg("46029ef5-0b73-4a71-bff3-d0d05de42aac") // This is our internal command to do the initial setup
            .status()
            .with_context(|| "Failed to start juliaup for the initial setup.")?;
    }
    Ok(())
}

fn run_versiondb_update(
    config_file: &juliaup::config_file::JuliaupReadonlyConfigFile,
) -> Result<()> {
    use chrono::Utc;
    use std::process::Stdio;

    let versiondb_update_interval = config_file.data.settings.versionsdb_update_interval;

    if versiondb_update_interval > 0 {
        let should_run =
            if let Some(last_versiondb_update) = config_file.data.last_version_db_update {
                let update_time =
                    last_versiondb_update + chrono::Duration::minutes(versiondb_update_interval);
                Utc::now() >= update_time
            } else {
                true
            };

        if should_run {
            let juliaup_path =
                get_juliaup_path().with_context(|| "Failed to obtain juliaup path.")?;

            std::process::Command::new(juliaup_path)
                .args(["0cf1528f-0b15-46b1-9ac9-e5bf5ccccbcf"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .spawn()
                .with_context(|| "Failed to start juliaup for version db update.")?;
        };
    }

    Ok(())
}

#[cfg(feature = "selfupdate")]
fn run_selfupdate(config_file: &juliaup::config_file::JuliaupReadonlyConfigFile) -> Result<()> {
    use chrono::Utc;
    use std::process::Stdio;

    if let Some(val) = config_file.self_data.startup_selfupdate_interval {
        let should_run = if let Some(last_selfupdate) = config_file.self_data.last_selfupdate {
            let update_time = last_selfupdate + chrono::Duration::minutes(val);

            Utc::now() >= update_time
        } else {
            true
        };

        if should_run {
            let juliaup_path =
                get_juliaup_path().with_context(|| "Failed to obtain juliaup path.")?;

            std::process::Command::new(juliaup_path)
                .args(["self", "update"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .spawn()
                .with_context(|| "Failed to start juliaup for self update.")?;
        };
    }

    Ok(())
}

#[cfg(not(feature = "selfupdate"))]
fn run_selfupdate(_config_file: &juliaup::config_file::JuliaupReadonlyConfigFile) -> Result<()> {
    Ok(())
}

fn is_interactive() -> bool {
    // First check if we have TTY access - this is a prerequisite for interactivity
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return false;
    }

    // Even with TTY available, check if Julia is being invoked in a non-interactive way
    let args: Vec<String> = std::env::args().collect();

    // Skip the first argument (program name) and any channel specification (+channel)
    let mut julia_args = args.iter().skip(1);

    // Skip channel specification if present
    if let Some(first_arg) = julia_args.clone().next() {
        if first_arg.starts_with('+') {
            julia_args.next(); // consume the +channel argument
        }
    }

    // Check for non-interactive usage patterns
    for arg in julia_args {
        match arg.as_str() {
            // Expression evaluation is non-interactive
            "-e" | "--eval" | "-E" | "--print" => return false,
            // Reading from stdin pipe is non-interactive
            "-" => return false,
            // Version display is non-interactive
            "-v" | "--version" => return false,
            // Help is non-interactive
            "-h" | "--help" | "--help-hidden" => return false,
            // Check if this looks like a Julia file (ends with .jl)
            filename if filename.ends_with(".jl") && !filename.starts_with('-') => {
                return false;
            }
            // Any other non-flag argument that doesn't start with '-' could be a script
            filename if !filename.starts_with('-') && !filename.is_empty() => {
                // This could be a script file, check if it exists as a file
                if std::path::Path::new(filename).exists() {
                    return false;
                }
            }
            _ => {} // Continue checking other arguments
        }
    }

    true
}

// Constants matching Julia's base/loading.jl
// https://github.com/JuliaLang/julia/blob/dd80509227adbd525737244ebabc95ec5d634354/base/loading.jl#L625C1-L631C2
const PROJECT_NAMES: &[&str] = &["JuliaProject.toml", "Project.toml"];
// excludes versioned manifests here
const MANIFEST_NAMES: &[&str] = &["JuliaManifest.toml", "Manifest.toml"];

/// Search upward from dir for a project file (Julia's current_project)
/// https://github.com/JuliaLang/julia/blob/dd80509227adbd525737244ebabc95ec5d634354/base/initdefs.jl#L203-L216
fn current_project(dir: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir();
    let mut current = dir;

    loop {
        // Check for project files in priority order
        for proj in PROJECT_NAMES {
            let file = current.join(proj);
            if file.exists() && file.is_file() {
                return Some(file);
            }
        }

        // Bail at home directory
        if let Some(ref home) = home {
            if current == home.as_path() {
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
fn load_path_expand_impl(
    env: &str,
    current_dir: &Path,
    depot_path: Option<&std::ffi::OsStr>,
) -> Result<Option<PathBuf>> {
    // Named environment?
    if let Some(stripped) = env.strip_prefix('@') {
        if stripped.is_empty() {
            // "@" - would need active_project, skip for now
            return Ok(None);
        } else if stripped == "." {
            // "@." - current project
            return Ok(current_project(current_dir));
        } else if stripped == "stdlib" {
            // "@stdlib" - skip, not relevant for version detection
            return Ok(None);
        }

        // Named environment like "@v1.10"
        let depot_paths = match depot_path {
            Some(paths) if !paths.is_empty() => std::env::split_paths(paths).collect::<Vec<_>>(),
            _ => {
                let home = dirs::home_dir().ok_or_else(|| {
                    anyhow!("Could not determine the path of the user home directory.")
                })?;
                vec![home.join(".julia")]
            }
        };

        // Look for named env in each depot
        for depot in &depot_paths {
            let path = depot.join("environments").join(stripped);
            if !path.exists() || !path.is_dir() {
                continue;
            }

            for proj in PROJECT_NAMES {
                let file = path.join(proj);
                if file.exists() && file.is_file() {
                    return Ok(Some(file));
                }
            }
        }

        // Return default location even if it doesn't exist
        if depot_paths.is_empty() {
            return Ok(None);
        }
        return Ok(Some(
            depot_paths[0]
                .join("environments")
                .join(stripped)
                .join(PROJECT_NAMES[PROJECT_NAMES.len() - 1]),
        ));
    }

    // Otherwise, it's a path
    let path = expand_tilde(env)?.into_owned();
    let path = if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    };

    if path.is_dir() {
        // Directory with a project file?
        for proj in PROJECT_NAMES {
            let file = path.join(proj);
            if file.exists() && file.is_file() {
                return Ok(Some(file));
            }
        }
    }

    // Package dir or path to project file
    Ok(Some(path))
}

/// Search JULIA_LOAD_PATH for the first valid project with a manifest
fn find_project_from_load_path(
    load_path: &str,
    current_dir: &Path,
    depot_path: Option<&std::ffi::OsStr>,
) -> Result<Option<PathBuf>> {
    let separator = if cfg!(windows) { ';' } else { ':' };

    for entry in load_path.split(separator) {
        let entry = entry.trim();

        // Skip empty entries and special entries that don't represent projects
        if entry.is_empty() || entry == "@" || entry.starts_with("@v") || entry == "@stdlib" {
            continue;
        }

        // Try to expand this load path entry as a project
        if let Ok(Some(project_file)) = load_path_expand_impl(entry, current_dir, depot_path) {
            // Check if this project has a manifest
            if project_file_manifest_path(&project_file).is_some() {
                log::debug!(
                    "VersionDetect::Found valid project in JULIA_LOAD_PATH entry: {}",
                    entry
                );
                return Ok(Some(project_file));
            }
        }
    }

    log::debug!("VersionDetect::No valid project with manifest found in JULIA_LOAD_PATH");
    Ok(None)
}

/// Check if a Julia option requires an argument (mimics getopt's required_argument)
/// Based on jloptions.c:421-493
fn julia_option_requires_arg(opt: &str) -> bool {
    // Options with = already include their argument
    if opt.contains('=') {
        return false;
    }

    // Handle short options: from shortopts = "+vhqH:e:E:L:J:C:it:p:O:g:m:"
    // Options WITHOUT ':' are no_argument, 'O' and 'g' are optional_argument
    if let Some(short) = opt.strip_prefix('-').and_then(|s| {
        if !s.starts_with('-') && s.len() == 1 {
            s.chars().next()
        } else {
            None
        }
    }) {
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
fn init_active_project_impl(
    args: &[String],
    current_dir: &Path,
    julia_project: Option<&str>,
    depot_path: Option<&std::ffi::OsStr>,
) -> Result<Option<PathBuf>> {
    // Check for --project flag in args
    // Stop parsing at "--" or the first positional argument (non-flag)
    // to match Julia's argument parsing behavior
    let mut project_cli: Option<Option<String>> = None;
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];

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
            index += 1;
        }
        index += 1;
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
fn project_file_manifest_path(project_file: &Path) -> Option<PathBuf> {
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

    // Then check standard manifests in priority order
    for mfst in MANIFEST_NAMES {
        let manifest_file = dir.join(mfst);
        if manifest_file.exists() && manifest_file.is_file() {
            return Some(manifest_file);
        }
    }

    None
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
fn determine_project_version_spec(args: &[String]) -> Result<Option<String>> {
    determine_project_version_spec_impl(
        args,
        std::env::var("JULIA_PROJECT").ok(),
        std::env::var("JULIA_LOAD_PATH").ok(),
        &std::env::current_dir().with_context(|| "Failed to determine current directory.")?,
    )
}

fn determine_project_version_spec_impl(
    args: &[String],
    julia_project: Option<String>,
    julia_load_path: Option<String>,
    current_dir: &Path,
) -> Result<Option<String>> {
    let depot_path = std::env::var_os("JULIA_DEPOT_PATH");

    // Get the active project file using Julia's init_active_project logic
    let project_file = match init_active_project_impl(
        args,
        current_dir,
        julia_project.as_deref(),
        depot_path.as_deref(),
    )? {
        Some(file) => file,
        None => {
            // If no project specified via --project or JULIA_PROJECT,
            // try searching JULIA_LOAD_PATH
            if let Some(load_path) = julia_load_path {
                match find_project_from_load_path(&load_path, current_dir, depot_path.as_deref())? {
                    Some(file) => file,
                    None => {
                        log::debug!("VersionDetect::No project specification found");
                        return Ok(None);
                    }
                }
            } else {
                log::debug!("VersionDetect::No project specification found");
                return Ok(None);
            }
        }
    };

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
fn find_highest_versioned_manifest(project_root: &Path) -> Option<PathBuf> {
    let Ok(entries) = fs::read_dir(project_root) else {
        return None;
    };

    // Track highest version for both JuliaManifest and Manifest separately
    let mut highest_julia: Option<(Version, PathBuf)> = None;
    let mut highest_manifest: Option<(Version, PathBuf)> = None;

    // Helper to update highest version if new version is greater
    let update_highest =
        |highest: &mut Option<(Version, PathBuf)>, version: Version, path: PathBuf| match highest {
            Some((current_version, _)) if version > *current_version => {
                *highest = Some((version, path));
            }
            None => {
                *highest = Some((version, path));
            }
            _ => {}
        };

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            // Check for versioned manifests: JuliaManifest-v*.toml or Manifest-v*.toml
            let (prefix, target) = if filename.starts_with("JuliaManifest-v") {
                ("JuliaManifest-v", &mut highest_julia)
            } else if filename.starts_with("Manifest-v") {
                ("Manifest-v", &mut highest_manifest)
            } else {
                continue;
            };

            if let Some(stripped) = filename.strip_prefix(prefix) {
                if let Some(version_str) = stripped.strip_suffix(".toml") {
                    if let Some(version) = parse_version_lenient(version_str) {
                        update_highest(target, version, path);
                    }
                }
            }
        }
    }

    // Return highest version, preferring JuliaManifest for same version
    match (highest_julia, highest_manifest) {
        (Some((jv, jpath)), Some((mv, mpath))) => {
            if jv >= mv {
                Some(jpath)
            } else {
                Some(mpath)
            }
        }
        (Some((_, jpath)), None) => Some(jpath),
        (None, Some((_, mpath))) => Some(mpath),
        (None, None) => None,
    }
}

// Parse a version string leniently, handling incomplete versions like "1.11" or "1"
fn parse_version_lenient(version_str: &str) -> Option<Version> {
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

fn read_manifest_julia_version(path: &Path) -> Result<Option<String>> {
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

fn parse_db_version(version: &str) -> Result<Version> {
    let base = version
        .split('+')
        .next()
        .ok_or_else(|| anyhow!("Invalid version string `{}`.", version))?;
    Version::parse(base).with_context(|| format!("Failed to parse version `{}`.", base))
}

fn max_available_version(versions_db: &JuliaupVersionDB) -> Result<Option<Version>> {
    let mut max_version: Option<Version> = None;
    for key in versions_db.available_versions.keys() {
        if let Ok(version) = parse_db_version(key) {
            max_version = match max_version {
                Some(current) if current >= version => Some(current),
                _ => Some(version),
            };
        }
    }
    Ok(max_version)
}

fn resolve_auto_channel(required: String, versions_db: &JuliaupVersionDB) -> Result<String> {
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
        let versioned_nightly = format!(
            "{}.{}-nightly",
            required_version.major, required_version.minor
        );

        if versions_db
            .available_channels
            .contains_key(&versioned_nightly)
        {
            eprintln!(
                "{} Manifest specifies prerelease Julia {}. Using {} channel.",
                style("Info:").cyan().bold(),
                required,
                versioned_nightly
            );
            return Ok(versioned_nightly);
        }

        // Fall back to main nightly channel
        eprintln!(
            "{} Manifest specifies prerelease Julia {}. Using nightly channel.",
            style("Info:").cyan().bold(),
            required
        );
        return Ok("nightly".to_string());
    }

    // Check if the requested version is higher than any known version for this minor series
    // This handles both regular versions (e.g., 1.12.55 > 1.12.1) and prerelease versions
    // (e.g., 1.12.0-rc1 when only 1.11.x exists)
    let max_version_for_minor =
        max_version_for_minor(versions_db, required_version.major, required_version.minor)?;

    if let Some(max_minor_version) = &max_version_for_minor {
        if &required_version > max_minor_version {
            // The requested version is higher than any known version for this minor series
            let channel = format!(
                "{}.{}-nightly",
                required_version.major, required_version.minor
            );
            eprintln!(
                "{} Manifest specifies Julia {} but the highest known version for {}.{} is {}. Using {} channel.",
                style("Info:").cyan().bold(),
                required,
                required_version.major,
                required_version.minor,
                max_minor_version,
                channel
            );
            return Ok(channel);
        }
    }

    // Check if requested version is higher than any known version overall
    // This handles the case where we have 1.12.x but request 1.13.0
    let max_known_version = max_available_version(versions_db)?;
    if let Some(max_version) = &max_known_version {
        if &required_version > max_version {
            // Check if a version-specific nightly channel exists (e.g., 1.13-nightly)
            let versioned_nightly = format!(
                "{}.{}-nightly",
                required_version.major, required_version.minor
            );

            if versions_db
                .available_channels
                .contains_key(&versioned_nightly)
            {
                eprintln!(
                    "{} Manifest specifies Julia {} but the highest known version is {}. Using {} channel.",
                    style("Info:").cyan().bold(),
                    required,
                    max_version,
                    versioned_nightly
                );
                return Ok(versioned_nightly);
            }

            // Fall back to main nightly channel
            eprintln!(
                "{} Manifest specifies Julia {} but the highest known version is {}. Using nightly channel.",
                style("Info:").cyan().bold(),
                required,
                max_version
            );
            return Ok("nightly".to_string());
        }
    } else {
        // No versions in database at all, use nightly
        eprintln!(
            "{} Manifest specifies Julia {} but no versions are known. Using nightly channel.",
            style("Info:").cyan().bold(),
            required
        );
        return Ok("nightly".to_string());
    }

    Err(anyhow!(
        "Julia version `{}` requested by Project.toml/Manifest.toml is not available in the versions database.",
        required
    ))
}

/// Find the maximum version (including prerelease) for a given major.minor series
fn max_version_for_minor(
    versions_db: &JuliaupVersionDB,
    major: u64,
    minor: u64,
) -> Result<Option<Version>> {
    let mut max_version: Option<Version> = None;

    // Check both available_versions and available_channels for the most complete picture
    for key in versions_db.available_channels.keys() {
        if let Ok(version) = parse_db_version(key) {
            if version.major == major && version.minor == minor {
                max_version = match &max_version {
                    Some(current) if current >= &version => Some(current.clone()),
                    _ => Some(version),
                };
            }
        }
    }

    Ok(max_version)
}

fn get_auto_channel(args: &[String], versions_db: &JuliaupVersionDB) -> Option<String> {
    determine_project_version_spec(args)
        .and_then(|opt_version| {
            opt_version
                .map(|version| resolve_auto_channel(version, versions_db))
                .transpose()
        })
        .ok()
        .flatten()
}

fn handle_auto_install_prompt(
    channel: &str,
    paths: &juliaup::global_paths::GlobalPaths,
) -> Result<bool> {
    // Check if we're in interactive mode
    if !is_interactive() {
        // Non-interactive mode, don't auto-install
        return Ok(false);
    }

    // Use dialoguer for a consistent UI experience
    let selection = Select::new()
        .with_prompt(format!(
            "{} The Juliaup channel '{}' is not installed. Would you like to install it?",
            style("Question:").yellow().bold(),
            channel
        ))
        .item("Yes (install this time only)")
        .item("Yes and remember my choice (always auto-install)")
        .item("No")
        .default(0) // Default to "Yes"
        .interact()?;

    match selection {
        0 => {
            // Just install for this time
            Ok(true)
        }
        1 => {
            // Install and remember the preference
            set_auto_install_preference(true, paths)?;
            Ok(true)
        }
        2 => {
            // Don't install
            Ok(false)
        }
        _ => {
            // Should not happen with dialoguer, but default to no
            Ok(false)
        }
    }
}

fn set_auto_install_preference(
    auto_install: bool,
    paths: &juliaup::global_paths::GlobalPaths,
) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "Failed to load configuration for setting auto-install preference.")?;

    config_file.data.settings.auto_install_channels = Some(auto_install);

    save_config_db(&mut config_file)
        .with_context(|| "Failed to save auto-install preference to configuration.")?;

    eprintln!(
        "{} Auto-install preference set to '{}'.",
        style("Info:").cyan().bold(),
        auto_install
    );

    Ok(())
}

fn spawn_juliaup_add(
    channel: &str,
    _paths: &juliaup::global_paths::GlobalPaths,
    is_automatic: bool,
) -> Result<()> {
    if is_automatic {
        eprintln!(
            "{} Installing Julia {} automatically per juliaup settings...",
            style("Info:").cyan().bold(),
            channel
        );
    } else {
        eprintln!(
            "{} Installing Julia {} as requested...",
            style("Info:").cyan().bold(),
            channel
        );
    }

    let juliaup_path = get_juliaup_path().with_context(|| "Failed to obtain juliaup path.")?;

    let status = std::process::Command::new(juliaup_path)
        .args(["add", channel])
        .status()
        .with_context(|| format!("Failed to spawn juliaup to install channel '{}'", channel))?;

    if status.success() {
        eprintln!(
            "{} Successfully installed Julia {}.",
            style("Info:").cyan().bold(),
            channel
        );
        Ok(())
    } else {
        Err(anyhow!(
            "Failed to install channel '{}'. juliaup add command failed with exit code: {:?}",
            channel,
            status.code()
        ))
    }
}

fn check_channel_uptodate(
    channel: &str,
    current_version: &str,
    versions_db: &JuliaupVersionDB,
) -> Result<()> {
    let latest_version = &versions_db
        .available_channels
        .get(channel)
        .ok_or_else(|| UserError {
            msg: format!(
                "The channel `{}` does not exist in the versions database.",
                channel
            ),
        })?
        .version;

    if latest_version != current_version {
        eprintln!("The latest version of Julia in the `{}` channel is {}. You currently have `{}` installed. Run:", channel, latest_version, current_version);
        eprintln!();
        eprintln!("  juliaup update");
        eprintln!();
        eprintln!(
            "in your terminal shell to install Julia {} and update the `{}` channel to that version.",
            latest_version, channel
        );
    }
    Ok(())
}

fn is_nightly_channel(channel: &str) -> bool {
    use regex::Regex;
    let nightly_re =
        Regex::new(r"^((?:nightly|latest)|(\d+\.\d+)-(?:nightly|latest))(~|$)").unwrap();
    nightly_re.is_match(channel)
}

enum JuliaupChannelSource {
    CmdLine,
    EnvVar,
    Override,
    Auto,
    Default,
}

fn get_julia_path_from_channel(
    versions_db: &JuliaupVersionDB,
    config_data: &JuliaupConfig,
    channel: &str,
    juliaupconfig_path: &Path,
    juliaup_channel_source: JuliaupChannelSource,
    paths: &juliaup::global_paths::GlobalPaths,
) -> Result<(PathBuf, Vec<String>)> {
    // First check if the channel is an alias and extract its args
    let (resolved_channel, alias_args) = match config_data.installed_channels.get(channel) {
        Some(JuliaupConfigChannel::AliasChannel { target, args }) => {
            (target.to_string(), args.clone().unwrap_or_default())
        }
        _ => (channel.to_string(), Vec::new()),
    };

    let channel_valid = is_valid_channel(versions_db, &resolved_channel)?;

    // First check if the channel is already installed
    if let Some(channel_info) = config_data.installed_channels.get(&resolved_channel) {
        return get_julia_path_from_installed_channel(
            versions_db,
            config_data,
            &resolved_channel,
            juliaupconfig_path,
            channel_info,
            alias_args.clone(),
        );
    }

    // Handle auto-installation for command line channel selection and auto-resolved channels
    if matches!(
        juliaup_channel_source,
        JuliaupChannelSource::CmdLine | JuliaupChannelSource::Auto
    ) && (channel_valid
        || is_pr_channel(&resolved_channel)
        || is_nightly_channel(&resolved_channel))
    {
        // Check the user's auto-install preference
        let should_auto_install = match config_data.settings.auto_install_channels {
            Some(auto_install) => auto_install, // User has explicitly set a preference
            None => {
                // User hasn't set a preference - prompt in interactive mode, default to false in non-interactive
                if is_interactive() {
                    handle_auto_install_prompt(&resolved_channel, paths)?
                } else {
                    false
                }
            }
        };

        if should_auto_install {
            // Install the channel using juliaup
            let is_automatic = config_data.settings.auto_install_channels == Some(true);
            spawn_juliaup_add(&resolved_channel, paths, is_automatic)?;

            // Reload the config to get the newly installed channel
            let updated_config_file = load_config_db(paths, None)
                .with_context(|| "Failed to reload configuration after installing channel.")?;

            let updated_channel_info = updated_config_file
                .data
                .installed_channels
                .get(&resolved_channel);

            if let Some(channel_info) = updated_channel_info {
                return get_julia_path_from_installed_channel(
                    versions_db,
                    &updated_config_file.data,
                    &resolved_channel,
                    juliaupconfig_path,
                    channel_info,
                    alias_args,
                );
            } else {
                return Err(anyhow!(
                        "Channel '{resolved_channel}' was installed but could not be found in configuration."
                    ));
            }
        }
        // If we reach here, either installation failed or user declined
    }

    // Original error handling for non-command-line sources or invalid channels
    let error = match juliaup_channel_source {
        JuliaupChannelSource::CmdLine => {
            if channel_valid {
                UserError { msg: format!("`{resolved_channel}` is not installed. Please run `juliaup add {resolved_channel}` to install channel or version.") }
            } else if is_pr_channel(&resolved_channel) {
                UserError { msg: format!("`{resolved_channel}` is not installed. Please run `juliaup add {resolved_channel}` to install pull request channel if available.") }
            } else if is_nightly_channel(&resolved_channel) {
                UserError { msg: format!("`{resolved_channel}` is not installed. Please run `juliaup add {resolved_channel}` to install nightly channel.") }
            } else {
                UserError { msg: format!("Invalid Juliaup channel `{resolved_channel}`. Please run `juliaup list` to get a list of valid channels and versions.") }
            }
        },
        JuliaupChannelSource::EnvVar=> {
            if channel_valid {
                UserError { msg: format!("`{resolved_channel}` from environment variable JULIAUP_CHANNEL is not installed. Please run `juliaup add {resolved_channel}` to install channel or version.") }
            } else if is_pr_channel(&resolved_channel) {
                UserError { msg: format!("`{resolved_channel}` from environment variable JULIAUP_CHANNEL is not installed. Please run `juliaup add {resolved_channel}` to install pull request channel if available.") }
            } else {
                UserError { msg: format!("Invalid Juliaup channel `{resolved_channel}` from environment variable JULIAUP_CHANNEL. Please run `juliaup list` to get a list of valid channels and versions.") }
            }
        },
        JuliaupChannelSource::Override=> {
            if channel_valid {
                UserError { msg: format!("`{resolved_channel}` from directory override is not installed. Please run `juliaup add {resolved_channel}` to install channel or version.") }
            } else if is_pr_channel(&resolved_channel) {
                UserError { msg: format!("`{resolved_channel}` from directory override is not installed. Please run `juliaup add {resolved_channel}` to install pull request channel if available.") }
            } else {
                UserError { msg: format!("Invalid Juliaup channel `{resolved_channel}` from directory override. Please run `juliaup list` to get a list of valid channels and versions.") }
            }
        },
        JuliaupChannelSource::Auto => {
            if channel_valid {
                UserError { msg: format!("`{resolved_channel}` resolved from project manifest is not installed. Please run `juliaup add {resolved_channel}` to install channel or version.") }
            } else if is_pr_channel(&resolved_channel) {
                UserError { msg: format!("`{resolved_channel}` resolved from project manifest is not installed. Please run `juliaup add {resolved_channel}` to install pull request channel if available.") }
            } else if is_nightly_channel(&resolved_channel) {
                UserError { msg: format!("`{resolved_channel}` resolved from project manifest is not installed. Please run `juliaup add {resolved_channel}` to install nightly channel.") }
            } else {
                UserError { msg: format!("Invalid Juliaup channel `{resolved_channel}` resolved from project manifest. Please run `juliaup list` to get a list of valid channels and versions.") }
            }
        },
        JuliaupChannelSource::Default => UserError {msg: format!("The Juliaup configuration is in an inconsistent state, the currently configured default channel `{resolved_channel}` is not installed.") }
    };

    Err(error.into())
}

fn get_julia_path_from_installed_channel(
    versions_db: &JuliaupVersionDB,
    config_data: &JuliaupConfig,
    channel: &str,
    juliaupconfig_path: &Path,
    channel_info: &JuliaupConfigChannel,
    alias_args: Vec<String>,
) -> Result<(PathBuf, Vec<String>)> {
    match channel_info {
        JuliaupConfigChannel::AliasChannel { .. } => {
            bail!("Unexpected alias channel after resolution: {channel}");
        }
        JuliaupConfigChannel::LinkedChannel { command, args } => {
            let mut combined_args = alias_args;
            combined_args.extend(args.as_ref().map_or_else(Vec::new, |v| v.clone()));
            Ok((PathBuf::from(command), combined_args))
        }
        JuliaupConfigChannel::SystemChannel { version } => {
            let path = &config_data
                .installed_versions.get(version)
                .ok_or_else(|| anyhow!("The juliaup configuration is in an inconsistent state, the channel {channel} is pointing to Julia version {version}, which is not installed."))?.path;

            if is_interactive() {
                check_channel_uptodate(channel, version, versions_db).with_context(|| {
                    format!("The Julia launcher failed while checking whether the channel {channel} is up-to-date.")
                })?;
            }

            let absolute_path = juliaupconfig_path
                .parent()
                .unwrap() // unwrap OK because there should always be a parent
                .join(path)
                .join("bin")
                .join(format!("julia{}", std::env::consts::EXE_SUFFIX))
                .normalize()
                .with_context(|| {
                    format!(
                        "Failed to normalize path for Julia binary, starting from `{}`.",
                        juliaupconfig_path.display()
                    )
                })?;
            Ok((absolute_path.into_path_buf(), alias_args))
        }
        JuliaupConfigChannel::DirectDownloadChannel {
            path,
            url: _,
            local_etag,
            server_etag,
            version: _,
        } => {
            if local_etag != server_etag && is_interactive() {
                if channel.starts_with("nightly") {
                    // Nightly is updateable several times per day so this message will show
                    // more often than not unless folks update a couple of times a day.
                    // Also, folks using nightly are typically more experienced and need
                    // less detailed prompting
                    eprintln!(
                        "A new `nightly` version is available. Install with `juliaup update`."
                    );
                } else {
                    eprintln!(
                        "A new version of Julia for the `{}` channel is available. Run:",
                        channel
                    );
                    eprintln!();
                    eprintln!("  juliaup update");
                    eprintln!();
                    eprintln!("to install the latest Julia for the `{}` channel.", channel);
                }
            }

            let absolute_path = juliaupconfig_path
                .parent()
                .unwrap()
                .join(path)
                .join("bin")
                .join(format!("julia{}", std::env::consts::EXE_SUFFIX))
                .normalize()
                .with_context(|| {
                    format!(
                        "Failed to normalize path for Julia binary, starting from `{}`.",
                        juliaupconfig_path.display()
                    )
                })?;
            Ok((absolute_path.into_path_buf(), alias_args))
        }
    }
}

fn get_override_channel(
    config_file: &juliaup::config_file::JuliaupReadonlyConfigFile,
) -> Result<Option<String>> {
    let curr_dir = std::env::current_dir()?.canonicalize()?;

    let juliaup_override = config_file
        .data
        .overrides
        .iter()
        .filter(|i| curr_dir.starts_with(&i.path))
        .sorted_by_key(|i| i.path.len())
        .next_back();

    match juliaup_override {
        Some(val) => Ok(Some(val.channel.clone())),
        None => Ok(None),
    }
}

/// Determines which channel to use based on the inputs.
/// This is the core channel selection logic used both in production and tests.
/// Returns (channel_name, source)
///
/// Priority order:
/// 1. Command line (+channel)
/// 2. Environment variable (JULIAUP_CHANNEL)
/// 3. Override (from config file)
/// 4. Auto-detection (from project manifest)
/// 5. Default (from config file)
fn determine_channel(
    args: &[String],
    env_channel: Option<String>,
    override_channel: Option<String>,
    default_channel: Option<String>,
    versions_db: &JuliaupVersionDB,
) -> Result<(String, JuliaupChannelSource)> {
    // Parse command line for +channel
    let mut channel_from_cmd_line: Option<String> = None;
    if args.len() > 1 {
        let first_arg = &args[1];
        if let Some(stripped) = first_arg.strip_prefix('+') {
            channel_from_cmd_line = Some(stripped.to_string());
        }
    }

    // Priority order
    if let Some(channel) = channel_from_cmd_line {
        Ok((channel, JuliaupChannelSource::CmdLine))
    } else if let Some(channel) = env_channel {
        Ok((channel, JuliaupChannelSource::EnvVar))
    } else if let Some(channel) = override_channel {
        Ok((channel, JuliaupChannelSource::Override))
    } else if let Some(channel) = get_auto_channel(args, versions_db) {
        Ok((channel, JuliaupChannelSource::Auto))
    } else if let Some(channel) = default_channel {
        Ok((channel, JuliaupChannelSource::Default))
    } else {
        Err(anyhow!("Failed to determine juliaup channel"))
    }
}

fn run_app() -> Result<i32> {
    if std::io::stdout().is_terminal() {
        // Set console title
        let term = Term::stdout();
        term.set_title("Julia");
    }

    let paths = get_paths().with_context(|| "Trying to load all global paths.")?;

    do_initial_setup(&paths.juliaupconfig)
        .with_context(|| "The Julia launcher failed to run the initial setup steps.")?;

    let config_file = load_config_db(&paths, None)
        .with_context(|| "The Julia launcher failed to load a configuration file.")?;

    let versiondb_data = load_versions_db(&paths)
        .with_context(|| "The Julia launcher failed to load a versions db.")?;

    // Determine which channel to use
    let args: Vec<String> = std::env::args().collect();
    let (julia_channel_to_use, juliaup_channel_source) = determine_channel(
        &args,
        std::env::var("JULIAUP_CHANNEL").ok(),
        get_override_channel(&config_file)?,
        config_file.data.default.clone(),
        &versiondb_data,
    )
    .with_context(|| "The Julia launcher failed to figure out which juliaup channel to use.")?;

    let (julia_path, julia_args) = get_julia_path_from_channel(
        &versiondb_data,
        &config_file.data,
        &julia_channel_to_use,
        &paths.juliaupconfig,
        juliaup_channel_source,
        &paths,
    )
    .with_context(|| {
        format!(
            "The Julia launcher failed to determine the command for the `{}` channel.",
            julia_channel_to_use
        )
    })?;

    let mut new_args: Vec<String> = Vec::new();

    for i in julia_args {
        new_args.push(i);
    }

    for (i, v) in args.iter().skip(1).enumerate() {
        if i > 0 || !v.starts_with('+') {
            new_args.push(v.clone());
        }
    }

    // On *nix platforms we replace the current process with the Julia one.
    // This simplifies use in e.g. debuggers, but requires that we fork off
    // a subprocess to do the selfupdate and versiondb update.
    #[cfg(not(windows))]
    match unsafe { fork() } {
        // NOTE: It is unsafe to perform async-signal-unsafe operations from
        // forked multithreaded programs, so for complex functionality like
        // selfupdate to work julialauncher needs to remain single-threaded.
        // Ref: https://docs.rs/nix/latest/nix/unistd/fn.fork.html#safety
        Ok(ForkResult::Parent { child, .. }) => {
            // wait for the daemon-spawning child to finish
            match waitpid(child, None) {
                Ok(WaitStatus::Exited(_, code)) => {
                    if code != 0 {
                        panic!("Could not fork (child process exited with code: {})", code)
                    }
                }
                Ok(_) => {
                    panic!("Could not fork (child process did not exit normally)");
                }
                Err(e) => {
                    panic!("Could not fork (error waiting for child process, {})", e);
                }
            }

            // replace the current process
            let _ = std::process::Command::new(&julia_path)
                .args(&new_args)
                .exec();

            // this is only ever reached if launching Julia fails
            panic!(
                "Could not launch Julia. Verify that there is a valid Julia binary at \"{}\".",
                julia_path.to_string_lossy()
            )
        }
        Ok(ForkResult::Child) => {
            // double-fork to prevent zombies
            match unsafe { fork() } {
                Ok(ForkResult::Parent { child: _, .. }) => {
                    // we don't do anything here so that this process can be
                    // reaped immediately
                }
                Ok(ForkResult::Child) => {
                    // this is where we perform the actual work. we don't do
                    // any typical daemon-y things (like detaching the TTY)
                    // so that any error output is still visible.

                    // We set a Ctrl-C handler here that just doesn't do anything, as we want the Julia child
                    // process to handle things.
                    ctrlc::set_handler(|| ())
                        .with_context(|| "Failed to set the Ctrl-C handler.")?;

                    run_versiondb_update(&config_file)
                        .with_context(|| "Failed to run version db update")?;

                    run_selfupdate(&config_file).with_context(|| "Failed to run selfupdate.")?;
                }
                Err(_) => panic!("Could not double-fork"),
            }

            Ok(0)
        }
        Err(_) => panic!("Could not fork"),
    }

    // On other platforms (i.e., Windows) we just spawn a subprocess
    #[cfg(windows)]
    {
        // We set a Ctrl-C handler here that just doesn't do anything, as we want the Julia child
        // process to handle things.
        ctrlc::set_handler(|| ()).with_context(|| "Failed to set the Ctrl-C handler.")?;

        let mut job_attr: windows::Win32::Security::SECURITY_ATTRIBUTES =
            windows::Win32::Security::SECURITY_ATTRIBUTES::default();
        let mut job_info: windows::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION =
            windows::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();

        job_attr.bInheritHandle = false.into();
        job_info.BasicLimitInformation.LimitFlags =
            windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_BREAKAWAY_OK
                | windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK
                | windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let job_handle = unsafe {
            windows::Win32::System::JobObjects::CreateJobObjectW(
                Some(&job_attr),
                windows::core::PCWSTR::null(),
            )
        }?;
        unsafe {
            SetInformationJobObject(
                job_handle,
                windows::Win32::System::JobObjects::JobObjectExtendedLimitInformation,
                &job_info as *const _ as *const std::os::raw::c_void,
                std::mem::size_of_val(&job_info) as u32,
            )
        }?;

        unsafe { AssignProcessToJobObject(job_handle, GetCurrentProcess()) }?;

        let mut child_process = std::process::Command::new(julia_path)
            .args(&new_args)
            .spawn()
            .with_context(|| "The Julia launcher failed to start Julia.")?; // TODO Maybe include the command we actually tried to start?

        // We ignore any error here, as that is what libuv also does, see the documentation
        // at https://github.com/libuv/libuv/blob/5ff1fc724f7f53d921599dbe18e6f96b298233f1/src/win/process.c#L1077
        let _ = unsafe {
            AssignProcessToJobObject(
                job_handle,
                std::mem::transmute::<RawHandle, windows::Win32::Foundation::HANDLE>(
                    child_process.as_raw_handle(),
                ),
            )
        };

        run_versiondb_update(&config_file).with_context(|| "Failed to run version db update")?;

        run_selfupdate(&config_file).with_context(|| "Failed to run selfupdate.")?;

        let status = child_process
            .wait()
            .with_context(|| "Failed to wait for Julia process to finish.")?;

        let code = match status.code() {
            Some(code) => code,
            None => {
                anyhow::bail!("There is no exit code, that should not be possible on Windows.");
            }
        };

        Ok(code)
    }
}

fn main() -> Result<std::process::ExitCode> {
    let client_status: std::prelude::v1::Result<i32, anyhow::Error>;

    {
        human_panic::setup_panic!(human_panic::Metadata::new(
            "Juliaup launcher",
            env!("CARGO_PKG_VERSION")
        )
        .support("https://github.com/JuliaLang/juliaup"));

        let env = env_logger::Env::new()
            .filter("JULIAUP_LOG")
            .write_style("JULIAUP_LOG_STYLE");
        env_logger::init_from_env(env);

        client_status = run_app();

        if let Err(err) = &client_status {
            if let Some(e) = err.downcast_ref::<UserError>() {
                eprintln!("{} {}", style("ERROR:").red().bold(), e.msg);

                return Ok(std::process::ExitCode::FAILURE);
            } else {
                return Err(client_status.unwrap_err());
            }
        }
    }

    // TODO https://github.com/rust-lang/rust/issues/111688 is finalized, we should use that instead of calling exit
    std::process::exit(client_status?);
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use juliaup::jsonstructs_versionsdb::{JuliaupVersionDBChannel, JuliaupVersionDBVersion};
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    // Helper to create a test directory with Project.toml
    fn create_test_project(dir: &Path, project_content: &str) -> PathBuf {
        let project_file = dir.join("Project.toml");
        fs::write(&project_file, project_content).unwrap();
        project_file
    }

    // Helper to create a manifest file
    fn create_manifest(dir: &Path, name: &str, julia_version: &str) {
        let manifest_file = dir.join(name);
        fs::write(
            &manifest_file,
            format!(r#"julia_version = "{}""#, julia_version),
        )
        .unwrap();
    }

    // Platform-specific path separator for JULIA_LOAD_PATH
    #[cfg(windows)]
    const LOAD_PATH_SEPARATOR: &str = ";";
    #[cfg(not(windows))]
    const LOAD_PATH_SEPARATOR: &str = ":";

    #[test]
    fn test_load_path_expand_named_environment_dot() {
        // Test @. resolves to current directory's Project.toml
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");

        let result = load_path_expand_impl("@.", temp_dir.path(), None).unwrap();

        assert!(result.is_some());
        // Canonicalize both paths for comparison (handles symlinks on macOS)
        assert_eq!(
            result.unwrap().canonicalize().unwrap(),
            project_file.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_load_path_expand_named_environment_depot() {
        // Test named environment like @v1.10 resolves to depot
        let temp_dir = TempDir::new().unwrap();
        let depot = temp_dir.path().join("depot");
        let env_dir = depot.join("environments").join("v1.10");
        fs::create_dir_all(&env_dir).unwrap();
        create_test_project(&env_dir, "name = \"TestEnv\"");

        let depot_path_str = depot.as_os_str();
        let result =
            load_path_expand_impl("@v1.10", temp_dir.path(), Some(depot_path_str)).unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap(), env_dir.join("Project.toml"));
    }

    #[test]
    fn test_load_path_expand_regular_path() {
        // Test regular path expansion
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("myproject");
        fs::create_dir(&project_dir).unwrap();
        let project_file = create_test_project(&project_dir, "name = \"MyProject\"");

        let result =
            load_path_expand_impl(project_dir.to_str().unwrap(), temp_dir.path(), None).unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap(), project_file);
    }

    #[test]
    fn test_determine_project_version_spec_from_project_flag() {
        // Test --project flag with explicit path (end-to-end: manifest -> channel)
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.5");

        let args = vec![
            "julia".to_string(),
            format!("--project={}", temp_dir.path().display()),
            "-e".to_string(),
            "1+1".to_string(),
        ];

        let versions_db = create_test_versions_db();
        let result =
            determine_channel(&args, None, None, Some("default".to_string()), &versions_db);

        assert!(result.is_ok());
        let (channel, source) = result.unwrap();
        assert_eq!(channel, "1.10.5");
        assert!(matches!(source, JuliaupChannelSource::Auto));
    }

    #[test]
    fn test_determine_project_version_spec_from_project_flag_no_value() {
        // Test --project (without value) searches upward (end-to-end: manifest -> channel)
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.11.0");

        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let args = vec![
            "julia".to_string(),
            "--project".to_string(),
            "-e".to_string(),
            "1+1".to_string(),
        ];

        let versions_db = create_test_versions_db();
        let result =
            determine_channel(&args, None, None, Some("default".to_string()), &versions_db);

        std::env::set_current_dir(old_dir).unwrap();

        assert!(result.is_ok());
        let (channel, source) = result.unwrap();
        assert_eq!(channel, "1.11.0");
        assert!(matches!(source, JuliaupChannelSource::Auto));
    }

    #[test]
    fn test_determine_project_version_spec_from_env_var() {
        // Test JULIA_PROJECT environment variable auto-detects version
        // (Low-level test for JULIA_PROJECT parsing - complementary to end-to-end tests)
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.11.3");

        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result = determine_project_version_spec_impl(
            &args,
            Some(temp_dir.path().to_string_lossy().to_string()),
            None,
            temp_dir.path(),
        )
        .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.11.3");
    }

    #[test]
    fn test_determine_project_version_spec_from_env_var_empty_searches_upward() {
        // Test JULIA_PROJECT="" (empty) searches upward like @.
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.11.0");

        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result =
            determine_project_version_spec_impl(&args, Some("".to_string()), None, temp_dir.path())
                .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.11.0");
    }

    #[test]
    fn test_project_flag_stops_at_script_argument() {
        // Test that --project parsing stops at the first positional argument (script name)
        // julia script.jl --project=@foo should NOT use @foo
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.0");

        let args = vec![
            "julia".to_string(),
            "script.jl".to_string(),
            "--project=@foo".to_string(), // This should be ignored
        ];

        let result = determine_project_version_spec_impl(&args, None, None, temp_dir.path());
        // Should return None since --project=@foo comes after script.jl
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_project_flag_stops_at_double_dash() {
        // Test that --project parsing stops at --
        // julia -- script.jl --project=@foo should NOT use @foo
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.0");

        let args = vec![
            "julia".to_string(),
            "--".to_string(),
            "script.jl".to_string(),
            "--project=@foo".to_string(), // This should be ignored
        ];

        let result = determine_project_version_spec_impl(&args, None, None, temp_dir.path());
        // Should return None since --project=@foo comes after --
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_project_flag_before_script_is_recognized() {
        // Test that --project BEFORE the script is properly recognized
        // julia --project=path script.jl --project=@foo
        // Should use the first --project (before script.jl)
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.5");

        let args = vec![
            "julia".to_string(),
            format!("--project={}", temp_dir.path().display()),
            "script.jl".to_string(),
            "--project=@foo".to_string(), // This should be ignored
        ];

        let versions_db = create_test_versions_db();
        let result =
            determine_channel(&args, None, None, Some("default".to_string()), &versions_db);

        assert!(result.is_ok());
        let (channel, source) = result.unwrap();
        assert_eq!(channel, "1.10.5");
        assert!(matches!(source, JuliaupChannelSource::Auto));
    }

    #[test]
    fn test_determine_project_version_spec_from_env_var_named_environment() {
        // Test JULIA_PROJECT=@. resolves to current directory
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.2");

        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result = determine_project_version_spec_impl(
            &args,
            Some("@.".to_string()),
            None,
            temp_dir.path(),
        )
        .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.10.2");
    }

    #[test]
    fn test_determine_project_version_spec_flag_overrides_env() {
        // Test that --project flag takes precedence over JULIA_PROJECT env var
        let temp_dir1 = TempDir::new().unwrap();
        create_test_project(temp_dir1.path(), "name = \"Project1\"");
        create_manifest(temp_dir1.path(), "Manifest.toml", "1.9.0");

        let temp_dir2 = TempDir::new().unwrap();
        create_test_project(temp_dir2.path(), "name = \"Project2\"");
        create_manifest(temp_dir2.path(), "Manifest.toml", "1.10.0");

        // But use --project to point to temp_dir2
        let args = vec![
            "julia".to_string(),
            format!("--project={}", temp_dir2.path().display()),
            "-e".to_string(),
            "1+1".to_string(),
        ];

        let result = determine_project_version_spec_impl(
            &args,
            Some(temp_dir1.path().to_string_lossy().to_string()), // JULIA_PROJECT=temp_dir1
            None,
            temp_dir2.path(),
        )
        .unwrap();
        assert!(result.is_some());
        // Should use the version from temp_dir2 (flag), not temp_dir1 (env)
        assert_eq!(result.unwrap(), "1.10.0");
    }

    #[test]
    fn test_determine_project_version_spec_no_project_specified() {
        // Test that None is returned when no project is specified
        let temp_dir = TempDir::new().unwrap();
        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result =
            determine_project_version_spec_impl(&args, None, None, temp_dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_determine_project_version_spec_from_load_path() {
        // Test JULIA_LOAD_PATH environment variable
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.12.0");

        let load_path = format!(
            "@{}{}{}@stdlib",
            LOAD_PATH_SEPARATOR,
            temp_dir.path().display(),
            LOAD_PATH_SEPARATOR
        );

        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result =
            determine_project_version_spec_impl(&args, None, Some(load_path), temp_dir.path())
                .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.12.0");
    }

    #[test]
    fn test_determine_project_version_spec_load_path_searches_first_valid() {
        // Test that JULIA_LOAD_PATH returns the first valid project
        let temp_dir1 = TempDir::new().unwrap();
        create_test_project(temp_dir1.path(), "name = \"Project1\"");
        create_manifest(temp_dir1.path(), "Manifest.toml", "1.11.5");

        let temp_dir2 = TempDir::new().unwrap();
        create_test_project(temp_dir2.path(), "name = \"Project2\"");
        create_manifest(temp_dir2.path(), "Manifest.toml", "1.10.3");

        let load_path = format!(
            "{}{}{}",
            temp_dir1.path().display(),
            LOAD_PATH_SEPARATOR,
            temp_dir2.path().display()
        );

        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result =
            determine_project_version_spec_impl(&args, None, Some(load_path), temp_dir1.path())
                .unwrap();
        assert!(result.is_some());
        // Should use version from temp_dir1 (first in LOAD_PATH)
        assert_eq!(result.unwrap(), "1.11.5");
    }

    #[test]
    fn test_determine_project_version_spec_project_overrides_load_path() {
        // Test that JULIA_PROJECT takes precedence over JULIA_LOAD_PATH
        let temp_dir1 = TempDir::new().unwrap();
        create_test_project(temp_dir1.path(), "name = \"Project1\"");
        create_manifest(temp_dir1.path(), "Manifest.toml", "1.9.2");

        let temp_dir2 = TempDir::new().unwrap();
        create_test_project(temp_dir2.path(), "name = \"Project2\"");
        create_manifest(temp_dir2.path(), "Manifest.toml", "1.10.4");

        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result = determine_project_version_spec_impl(
            &args,
            Some(temp_dir2.path().to_string_lossy().to_string()), // JULIA_PROJECT
            Some(temp_dir1.path().to_string_lossy().to_string()), // JULIA_LOAD_PATH
            temp_dir2.path(),
        )
        .unwrap();
        assert!(result.is_some());
        // Should use version from temp_dir2 (JULIA_PROJECT), not temp_dir1 (JULIA_LOAD_PATH)
        assert_eq!(result.unwrap(), "1.10.4");
    }

    #[test]
    fn test_determine_project_version_spec_relative_paths() {
        // Test that relative paths work like Julia (relative to current directory)
        // Similar to: JULIA_LOAD_PATH="Pkg.jl" julia --project=DataFrames.jl -e '...'
        let parent_dir = TempDir::new().unwrap();

        // Create two project directories
        let pkg_dir = parent_dir.path().join("Pkg.jl");
        fs::create_dir(&pkg_dir).unwrap();
        create_test_project(&pkg_dir, "name = \"Pkg\"");
        create_manifest(&pkg_dir, "Manifest.toml", "1.9.0");

        let df_dir = parent_dir.path().join("DataFrames.jl");
        fs::create_dir(&df_dir).unwrap();
        create_test_project(&df_dir, "name = \"DataFrames\"");
        create_manifest(&df_dir, "Manifest.toml", "1.11.0");

        // Test 1: --project=DataFrames.jl should override JULIA_LOAD_PATH
        let args = vec![
            "julia".to_string(),
            "--project=DataFrames.jl".to_string(),
            "-e".to_string(),
            "1+1".to_string(),
        ];
        let result = determine_project_version_spec_impl(
            &args,
            None,
            Some("Pkg.jl".to_string()),
            parent_dir.path(),
        )
        .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.11.0");

        // Test 2: Without --project, should use JULIA_LOAD_PATH (Pkg.jl)
        let args2 = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];
        let result2 = determine_project_version_spec_impl(
            &args2,
            None,
            Some("Pkg.jl".to_string()),
            parent_dir.path(),
        )
        .unwrap();
        assert!(result2.is_some());
        assert_eq!(result2.unwrap(), "1.9.0");
    }

    #[test]
    fn test_project_parsing_with_required_arg_options() {
        // Test that --project parsing correctly handles options with required arguments (getopt behavior)
        // E.g., "julia --module --project foo.jl" treats "--project" as the module name, not a flag
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.5");

        // --project consumed by --module (long option)
        assert!(determine_project_version_spec_impl(
            &["julia", "--module", "--project", "foo.jl"]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
            None,
            None,
            temp_dir.path()
        )
        .unwrap()
        .is_none());

        // --project consumed by -e (short option)
        assert!(determine_project_version_spec_impl(
            &["julia", "-e", "--project", "foo.jl"]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
            None,
            None,
            temp_dir.path()
        )
        .unwrap()
        .is_none());

        // --project after multiple required args still works
        assert_eq!(
            determine_project_version_spec_impl(
                &[
                    "julia",
                    "--eval",
                    "1+1",
                    &format!("--project={}", temp_dir.path().display())
                ]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
                None,
                None,
                temp_dir.path()
            )
            .unwrap(),
            Some("1.10.5".to_string())
        );

        // Options with = don't consume next token
        assert_eq!(
            determine_project_version_spec_impl(
                &[
                    "julia",
                    "--eval=1+1",
                    &format!("--project={}", temp_dir.path().display())
                ]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
                None,
                None,
                temp_dir.path()
            )
            .unwrap(),
            Some("1.10.5".to_string())
        );
    }

    #[test]
    fn test_current_project_direct_search() {
        // Test current_project finds Project.toml in directory
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");

        let result = current_project(temp_dir.path());
        assert!(result.is_some());
        assert_eq!(result.unwrap(), project_file);
    }

    #[test]
    fn test_current_project_search_upward() {
        // Test current_project searches upward for Project.toml
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");

        // Create a subdirectory
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();

        let result = current_project(&subdir);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), project_file);
    }

    #[test]
    fn test_current_project_julia_project_precedence() {
        // Test that JuliaProject.toml takes precedence over Project.toml
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        let julia_project_file = temp_dir.path().join("JuliaProject.toml");
        fs::write(&julia_project_file, "name = \"JuliaTestProject\"").unwrap();

        let result = current_project(temp_dir.path());
        assert!(result.is_some());
        assert_eq!(result.unwrap(), julia_project_file);
    }

    #[test]
    fn test_current_project_not_found() {
        // Test when no project file exists
        let temp_dir = TempDir::new().unwrap();
        let result = current_project(temp_dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_project_file_manifest_path_default() {
        // Test default Manifest.toml detection
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.0");

        let result = project_file_manifest_path(&project_file);

        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "Manifest.toml");
    }

    #[test]
    fn test_project_file_manifest_path_julia_manifest_precedence() {
        // Test that JuliaManifest.toml takes precedence over Manifest.toml
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.0");
        create_manifest(temp_dir.path(), "JuliaManifest.toml", "1.11.0");

        let result = project_file_manifest_path(&project_file);

        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "JuliaManifest.toml");
    }

    #[test]
    fn test_project_file_manifest_path_versioned_manifest() {
        // Test versioned manifest detection
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");

        let result = project_file_manifest_path(&project_file);

        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "Manifest-v1.11.toml");
    }

    #[test]
    fn test_determine_manifest_path_multiple_versioned_manifests() {
        // Test that the highest versioned manifest is selected
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest-v1.10.toml", "1.10.0");
        create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");
        create_manifest(temp_dir.path(), "Manifest-v1.12.toml", "1.12.0");

        let result = project_file_manifest_path(&project_file);

        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "Manifest-v1.12.toml");
    }

    #[test]
    fn test_versioned_manifest_priority_over_standard() {
        // Test that versioned manifests take precedence over standard manifests
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.13.0");
        create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");
        create_manifest(temp_dir.path(), "Manifest-v1.12.toml", "1.12.0");

        let result = project_file_manifest_path(&project_file);

        assert!(result.is_some());
        // Versioned manifest should be selected (highest version)
        assert_eq!(result.unwrap().file_name().unwrap(), "Manifest-v1.12.toml");
    }

    #[test]
    fn test_julia_manifest_priority_over_manifest() {
        // Test that JuliaManifest-v*.toml takes precedence over Manifest-v*.toml for same version
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "JuliaManifest-v1.11.toml", "1.11.0");
        create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");

        let result = project_file_manifest_path(&project_file);

        assert!(result.is_some());
        assert_eq!(
            result.unwrap().file_name().unwrap(),
            "JuliaManifest-v1.11.toml"
        );
    }

    #[test]
    fn test_higher_version_wins_regardless_of_prefix() {
        // Test that higher version wins even if it's Manifest (not JuliaManifest)
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "JuliaManifest-v1.10.toml", "1.10.0");
        create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");

        let result = project_file_manifest_path(&project_file);

        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "Manifest-v1.11.toml");
    }

    #[test]
    fn test_determine_manifest_path_explicit_manifest_field() {
        // Test explicit manifest field in Project.toml
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(
            temp_dir.path(),
            indoc! {r#"
                name = "TestProject"
                manifest = "custom/Manifest.toml"
            "#},
        );

        let custom_dir = temp_dir.path().join("custom");
        fs::create_dir(&custom_dir).unwrap();
        create_manifest(&custom_dir, "Manifest.toml", "1.10.0");

        let result = project_file_manifest_path(&project_file);

        assert!(result.is_some());
        assert!(result.unwrap().ends_with("custom/Manifest.toml"));
    }

    #[test]
    fn test_read_manifest_julia_version() {
        // Test reading julia_version from manifest
        let temp_dir = TempDir::new().unwrap();
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.5");

        let manifest_path = temp_dir.path().join("Manifest.toml");
        let result = read_manifest_julia_version(&manifest_path).unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.10.5");
    }

    #[test]
    fn test_read_manifest_julia_version_missing_file() {
        // Test reading from non-existent manifest
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("NonExistent.toml");

        let result = read_manifest_julia_version(&manifest_path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_manifest_julia_version_missing_field() {
        // Test reading manifest without julia_version field
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("Manifest.toml");
        fs::write(&manifest_path, "[deps]\nExample = \"1.0.0\"").unwrap();

        let result = read_manifest_julia_version(&manifest_path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_find_highest_versioned_manifest() {
        // Test finding highest versioned manifest
        let temp_dir = TempDir::new().unwrap();
        create_manifest(temp_dir.path(), "Manifest-v1.8.0.toml", "1.8.0");
        create_manifest(temp_dir.path(), "Manifest-v1.10.5.toml", "1.10.5");
        create_manifest(temp_dir.path(), "Manifest-v1.11.2.toml", "1.11.2");

        let result = find_highest_versioned_manifest(temp_dir.path());
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().file_name().unwrap(),
            "Manifest-v1.11.2.toml"
        );
    }

    #[test]
    fn test_find_highest_versioned_manifest_none() {
        // Test when no versioned manifests exist
        let temp_dir = TempDir::new().unwrap();
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.0");

        let result = find_highest_versioned_manifest(temp_dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_find_highest_versioned_manifest_invalid_names() {
        // Test that invalid versioned manifest names are ignored
        let temp_dir = TempDir::new().unwrap();
        create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");
        fs::write(temp_dir.path().join("Manifest-vInvalid.toml"), "invalid").unwrap();
        fs::write(temp_dir.path().join("Manifest-v.toml"), "invalid").unwrap();

        let result = find_highest_versioned_manifest(temp_dir.path());
        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "Manifest-v1.11.toml");
    }

    #[test]
    fn test_resolve_auto_channel_high_patch_version() {
        // Test that a patch version higher than any known minor version uses X.Y-nightly
        let versions_db = TestVersionsDbBuilder::new()
            .add_version("1.12.0")
            .add_channel("1.12.0", "1.12.0")
            .add_version("1.12.1")
            .add_channel("1.12.1", "1.12.1")
            .build();

        // Test 1: Version 1.12.55 (higher patch than any known) should resolve to 1.12-nightly
        let result = resolve_auto_channel("1.12.55".to_string(), &versions_db);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "1.12-nightly");

        // Test 2: Version 1.12.1 (exact match) should resolve to itself
        let result = resolve_auto_channel("1.12.1".to_string(), &versions_db);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "1.12.1");

        // Test 3: Version 1.12.0 (exact match) should resolve to itself
        let result = resolve_auto_channel("1.12.0".to_string(), &versions_db);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "1.12.0");
    }

    #[test]
    fn test_resolve_auto_channel_higher_than_any_version() {
        // Test that a version higher than any known version uses nightly
        let versions_db = TestVersionsDbBuilder::new()
            .add_version("1.12.0")
            .add_channel("1.12.0", "1.12.0")
            .add_version("1.12.1")
            .add_channel("1.12.1", "1.12.1")
            .build();

        // Version 1.13.0 (higher than any known version) should resolve to nightly
        let result = resolve_auto_channel("1.13.0".to_string(), &versions_db);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "nightly");
    }

    #[test]
    fn test_resolve_auto_channel_prerelease_versions() {
        // Test that prerelease versions use nightly channels appropriately
        let versions_db = TestVersionsDbBuilder::new()
            .add_version("1.11.0")
            .add_channel("1.11.0", "1.11.0")
            .add_version("1.12.1")
            .add_channel("1.12.1", "1.12.1")
            .add_channel("1.12.0-rc1", "1.12.0-rc1")
            .add_channel("1.12-nightly", "1.12.2-DEV")
            .add_channel("1.13-nightly", "1.13.0-DEV")
            .build();

        // Test 1: Exact match - 1.12.0-rc1 exists, so use it
        let result = resolve_auto_channel("1.12.0-rc1".to_string(), &versions_db);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "1.12.0-rc1");

        // Test 2: Exact match for prerelease of existing stable - 1.12.1-rc1 exists, so use it
        // Add 1.12.1-rc1 channel to test this case
        let versions_db_with_rc = TestVersionsDbBuilder::new()
            .add_version("1.11.0")
            .add_channel("1.11.0", "1.11.0")
            .add_version("1.12.1")
            .add_channel("1.12.1", "1.12.1")
            .add_channel("1.12.1-rc1", "1.12.1-rc1") // Prerelease of stable version
            .add_channel("1.12-nightly", "1.12.2-DEV")
            .build();

        let result = resolve_auto_channel("1.12.1-rc1".to_string(), &versions_db_with_rc);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "1.12.1-rc1");

        // Test 3: CRITICAL - 1.12.1-DEV < 1.12.1 in SemVer ordering, but should still use nightly
        // This is the common case when a manifest is generated on nightly
        let result = resolve_auto_channel("1.12.1-DEV".to_string(), &versions_db);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "1.12-nightly");

        // Test 4: 1.13.0-DEV should use 1.13-nightly
        let result = resolve_auto_channel("1.13.0-DEV".to_string(), &versions_db);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "1.13-nightly");

        // Test 5: 1.14.0-DEV (no 1.14-nightly exists), should use main nightly
        let result = resolve_auto_channel("1.14.0-DEV".to_string(), &versions_db);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "nightly");
    }

    // Helper to build a test versions database
    struct TestVersionsDbBuilder {
        available_versions: HashMap<String, JuliaupVersionDBVersion>,
        available_channels: HashMap<String, JuliaupVersionDBChannel>,
    }

    impl TestVersionsDbBuilder {
        fn new() -> Self {
            Self {
                available_versions: HashMap::new(),
                available_channels: HashMap::new(),
            }
        }

        fn add_version(mut self, version: &str) -> Self {
            self.available_versions.insert(
                version.to_string(),
                JuliaupVersionDBVersion {
                    url_path: "test".to_string(),
                },
            );
            self
        }

        fn add_channel(mut self, channel: &str, version: &str) -> Self {
            self.available_channels.insert(
                channel.to_string(),
                JuliaupVersionDBChannel {
                    version: version.to_string(),
                },
            );
            self
        }

        fn build(self) -> JuliaupVersionDB {
            JuliaupVersionDB {
                available_versions: self.available_versions,
                available_channels: self.available_channels,
                version: "1".to_string(),
            }
        }
    }

    // Helper to create a minimal versions db for testing
    fn create_test_versions_db() -> JuliaupVersionDB {
        TestVersionsDbBuilder::new()
            .add_version("1.10.0")
            .add_channel("1.10.0", "1.10.0")
            .add_version("1.10.5")
            .add_channel("1.10.5", "1.10.5")
            .add_version("1.11.0")
            .add_channel("1.11.0", "1.11.0")
            .add_version("1.11.3")
            .add_channel("1.11.3", "1.11.3")
            .build()
    }

    #[test]
    fn test_channel_selection_priority_cmdline_wins() {
        // Test that +channel has highest priority
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.5");

        let args = vec![
            "julia".to_string(),
            "+1.11.3".to_string(),
            format!("--project={}", temp_dir.path().display()),
            "-e".to_string(),
            "1+1".to_string(),
        ];

        let versions_db = create_test_versions_db();
        let result = determine_channel(
            &args,
            Some("1.10.0".to_string()),
            Some("override".to_string()),
            Some("default".to_string()),
            &versions_db,
        );

        assert!(result.is_ok());
        let (channel, source) = result.unwrap();
        assert_eq!(channel, "1.11.3");
        assert!(matches!(source, JuliaupChannelSource::CmdLine));
    }

    #[test]
    fn test_channel_selection_priority_env_over_auto() {
        // Test that JULIAUP_CHANNEL has priority over auto-detected version
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.5");

        let args = vec![
            "julia".to_string(),
            format!("--project={}", temp_dir.path().display()),
            "-e".to_string(),
            "1+1".to_string(),
        ];

        let versions_db = create_test_versions_db();
        let result = determine_channel(
            &args,
            Some("1.11.3".to_string()),
            None,
            Some("default".to_string()),
            &versions_db,
        );

        assert!(result.is_ok());
        let (channel, source) = result.unwrap();
        assert_eq!(channel, "1.11.3");
        assert!(matches!(source, JuliaupChannelSource::EnvVar));
    }

    #[test]
    fn test_channel_selection_auto_from_manifest() {
        // Test that auto-detection works when no higher priority source
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.5");

        let args = vec![
            "julia".to_string(),
            format!("--project={}", temp_dir.path().display()),
            "-e".to_string(),
            "1+1".to_string(),
        ];

        let versions_db = create_test_versions_db();
        let result =
            determine_channel(&args, None, None, Some("default".to_string()), &versions_db);

        assert!(result.is_ok());
        let (channel, source) = result.unwrap();
        assert_eq!(channel, "1.10.5");
        assert!(matches!(source, JuliaupChannelSource::Auto));
    }

    #[test]
    fn test_channel_selection_default_fallback() {
        // Test that default channel is used when nothing else applies
        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let versions_db = create_test_versions_db();
        let result =
            determine_channel(&args, None, None, Some("release".to_string()), &versions_db);

        assert!(result.is_ok());
        let (channel, source) = result.unwrap();
        assert_eq!(channel, "release");
        assert!(matches!(source, JuliaupChannelSource::Default));
    }

    #[test]
    fn test_channel_selection_override_priority() {
        // Test that override has priority over auto and default
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.5");

        let args = vec![
            "julia".to_string(),
            format!("--project={}", temp_dir.path().display()),
            "-e".to_string(),
            "1+1".to_string(),
        ];

        let versions_db = create_test_versions_db();
        let result = determine_channel(
            &args,
            None,
            Some("1.11.0".to_string()),
            Some("default".to_string()),
            &versions_db,
        );

        assert!(result.is_ok());
        let (channel, source) = result.unwrap();
        assert_eq!(channel, "1.11.0");
        assert!(matches!(source, JuliaupChannelSource::Override));
    }

    #[test]
    fn test_channel_selection_auto_with_project_flag_no_value() {
        // Test auto-detection with --project (no value) searches upward
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.11.0");

        // Change to temp directory for the test
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let args = vec![
            "julia".to_string(),
            "--project".to_string(),
            "-e".to_string(),
            "1+1".to_string(),
        ];

        let versions_db = create_test_versions_db();
        let result =
            determine_channel(&args, None, None, Some("default".to_string()), &versions_db);

        // Restore directory
        std::env::set_current_dir(old_dir).unwrap();

        assert!(result.is_ok());
        let (channel, source) = result.unwrap();
        assert_eq!(channel, "1.11.0");
        assert!(matches!(source, JuliaupChannelSource::Auto));
    }
}
