use anyhow::{anyhow, bail, Context, Result};
use console::{style, Term};
use dialoguer::Select;
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
    let mut project_spec_cli: Option<Option<String>> = None;
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];
        // Julia accepts abbreviated forms: --project, --projec, --proje, --proj
        // Note: --project without = always means "search upward" (no value)
        // Only --project=path specifies a value
        if ["--project", "--projec", "--proje", "--proj"].contains(&arg.as_str()) {
            project_spec_cli = Some(None);
        } else {
            for prefix in ["--project=", "--projec=", "--proje=", "--proj="] {
                if let Some(value) = arg.strip_prefix(prefix) {
                    project_spec_cli = Some(Some(value.to_string()));
                    break;
                }
            }
        }
        index += 1;
    }

    // Determine project spec in priority order:
    // 1. --project flag (from command line)
    // 2. JULIA_PROJECT environment variable
    // 3. JULIA_LOAD_PATH environment variable (search for first valid project)
    let project_spec = if let Some(spec) = project_spec_cli {
        Some(spec.unwrap_or_else(|| "@.".to_string()))
    } else if let Ok(env_spec) = std::env::var("JULIA_PROJECT") {
        if env_spec.trim().is_empty() {
            Some("@.".to_string())
        } else {
            Some(env_spec)
        }
    } else if let Ok(load_path) = std::env::var("JULIA_LOAD_PATH") {
        // Search through JULIA_LOAD_PATH for the first valid project
        find_project_from_load_path(&load_path)?
    } else {
        None
    };

    let Some(project_spec) = project_spec else {
        // No project specified - return None to allow fallback to normal channel resolution
        log::debug!("AutoVersionDetect::No project specification found (no --project flag, JULIA_PROJECT, or JULIA_LOAD_PATH)");
        return Ok(None);
    };

    log::debug!(
        "AutoVersionDetect::Using project specification: {}",
        project_spec
    );
    let project_file = resolve_project_location(&project_spec)?;
    let Some(project_file) = project_file else {
        // No project file found - silently fall back to release channel
        log::debug!(
            "AutoVersionDetect::No project file found for specification: {}",
            project_spec
        );
        return Ok(None);
    };

    let project_toml = match fs::read_to_string(&project_file) {
        Ok(contents) => contents,
        Err(err) => {
            return Err(anyhow!(
                "Failed to read Project.toml at `{}`: {}.",
                project_file.display(),
                err
            ));
        }
    };

    let parsed_project: Value = match toml::from_str(&project_toml) {
        Ok(value) => value,
        Err(err) => {
            return Err(anyhow!(
                "Failed to parse Project.toml at `{}`: {}.",
                project_file.display(),
                err
            ));
        }
    };

    if let Some(manifest_path) = determine_manifest_path(&project_file, &parsed_project) {
        log::debug!(
            "AutoVersionDetect::Detected manifest file: {}",
            manifest_path.display()
        );
        if let Some(version) = read_manifest_julia_version(&manifest_path)? {
            log::debug!(
                "AutoVersionDetect::Read Julia version from manifest: {}",
                version
            );
            return Ok(Some(version));
        } else {
            log::debug!(
                "AutoVersionDetect::Manifest file exists but does not contain julia_version field"
            );
        }
    } else {
        log::debug!("AutoVersionDetect::No manifest file found for project");
    }

    // No manifest with julia_version found, fall back to release
    Ok(None)
}

fn find_project_from_load_path(load_path: &str) -> Result<Option<String>> {
    // Parse JULIA_LOAD_PATH similar to how Julia does it
    // Split on ':' (Unix) or ';' (Windows)
    let separator = if cfg!(windows) { ';' } else { ':' };

    for entry in load_path.split(separator) {
        let entry = entry.trim();

        // Skip empty entries and special entries
        if entry.is_empty() || entry == "@" || entry.starts_with("@v") || entry == "@stdlib" {
            continue;
        }

        // Handle @. specially - it means current directory
        let entry_to_check = if entry == "@." {
            "@."
        } else if entry.starts_with('@') {
            // Other named environments - we could support these, but for now skip
            continue;
        } else {
            entry
        };

        // Try to resolve this as a project location
        if let Some(_project_file) = resolve_project_location(entry_to_check)? {
            // Found a valid project
            log::debug!(
                "AutoVersionDetect::Found valid project in JULIA_LOAD_PATH entry: {}",
                entry_to_check
            );
            return Ok(Some(entry_to_check.to_string()));
        }
    }

    // No valid project found in JULIA_LOAD_PATH
    log::debug!("AutoVersionDetect::No valid project found in JULIA_LOAD_PATH");
    Ok(None)
}

fn resolve_project_location(spec: &str) -> Result<Option<PathBuf>> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return resolve_project_location("@.");
    }

    if let Some(stripped) = trimmed.strip_prefix('@') {
        resolve_named_environment(stripped)
    } else {
        let path = PathBuf::from(trimmed);
        resolve_path_to_project(&path)
    }
}

fn resolve_named_environment(name: &str) -> Result<Option<PathBuf>> {
    let target_path = if name == "." {
        std::env::current_dir().with_context(|| "Failed to determine current directory.")?
    } else {
        let depot_paths = match std::env::var_os("JULIA_DEPOT_PATH") {
            Some(paths) if !paths.is_empty() => std::env::split_paths(&paths).collect(),
            _ => {
                let home = dirs::home_dir().ok_or_else(|| {
                    anyhow!("Could not determine the path of the user home directory.")
                })?;
                vec![home.join(".julia")]
            }
        };

        let mut candidate: Option<PathBuf> = None;
        for depot in depot_paths {
            let env_path = depot.join("environments").join(name);
            if env_path.exists() {
                candidate = Some(env_path);
                break;
            } else if candidate.is_none() {
                candidate = Some(env_path);
            }
        }

        candidate.ok_or_else(|| {
            anyhow!(
                "Failed to resolve environment `@{}` because no depot paths could be determined.",
                name
            )
        })?
    };

    resolve_path_to_project(&target_path)
}

fn resolve_path_to_project(path: &Path) -> Result<Option<PathBuf>> {
    let base_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .with_context(|| "Failed to determine current directory.")?
            .join(path)
    };

    // If the path is a file, use it directly
    if base_path.is_file() {
        log::debug!(
            "AutoVersionDetect::Using project file directly: {}",
            base_path.display()
        );
        return Ok(Some(base_path));
    }

    // If the path doesn't exist, don't search upward - return None
    if !base_path.exists() {
        log::debug!(
            "AutoVersionDetect::Project path `{}` does not exist.",
            base_path.display()
        );
        return Ok(None);
    }

    // If it's a directory, search upward from base_path for JuliaProject.toml or Project.toml
    // JuliaProject.toml takes precedence over Project.toml
    let mut current = base_path.as_path();
    loop {
        // Check for JuliaProject.toml first
        let julia_project_file = current.join("JuliaProject.toml");
        if julia_project_file.exists() {
            log::debug!(
                "AutoVersionDetect::Found JuliaProject.toml at: {}",
                julia_project_file.display()
            );
            return Ok(Some(julia_project_file));
        }

        // Fall back to Project.toml
        let project_file = current.join("Project.toml");
        if project_file.exists() {
            log::debug!(
                "AutoVersionDetect::Found Project.toml at: {}",
                project_file.display()
            );
            return Ok(Some(project_file));
        }

        // Move to parent directory
        match current.parent() {
            Some(parent) => current = parent,
            None => {
                // Reached filesystem root without finding project file
                log::debug!(
                    "AutoVersionDetect::No Project.toml or JuliaProject.toml found searching upward from `{}`.",
                    base_path.display()
                );
                return Ok(None);
            }
        }
    }
}

fn determine_manifest_path(project_file: &Path, project: &Value) -> Option<PathBuf> {
    let project_root = project_file.parent()?;

    // If project explicitly specifies manifest location, use that
    match project.get("manifest") {
        Some(Value::String(path)) => {
            if path.trim().is_empty() {
                return None;
            } else {
                let manifest_path = PathBuf::from(path);
                return Some(if manifest_path.is_absolute() {
                    manifest_path
                } else {
                    project_root.join(manifest_path)
                });
            }
        }
        Some(_) => {
            // Invalid manifest field type, fall through to default search
        }
        None => {}
    }

    // Search for manifest files in priority order:
    // 1. JuliaManifest.toml takes precedence over Manifest.toml
    // 2. Manifest.toml
    // 3. Versioned manifests (Manifest-v*.toml) - use the one with highest version
    // 4. Default to Manifest.toml if nothing exists (for error reporting)

    // Check for JuliaManifest.toml first
    let julia_manifest_path = project_root.join("JuliaManifest.toml");
    if julia_manifest_path.exists() {
        log::debug!(
            "AutoVersionDetect::Using JuliaManifest.toml (takes precedence over other manifests)"
        );
        return Some(julia_manifest_path);
    }

    // Check for Manifest.toml second
    let manifest_path = project_root.join("Manifest.toml");
    if manifest_path.exists() {
        log::debug!("AutoVersionDetect::Using Manifest.toml");
        return Some(manifest_path);
    }

    // Search for versioned manifests (e.g., Manifest-v1.11.toml, Manifest-v1.12.toml)
    // and use the one with the greatest version
    if let Some(versioned_manifest) = find_highest_versioned_manifest(project_root) {
        log::debug!(
            "AutoVersionDetect::Using versioned manifest: {}",
            versioned_manifest.display()
        );
        return Some(versioned_manifest);
    }

    // Default to Manifest.toml even if it doesn't exist (for error reporting)
    log::debug!(
        "AutoVersionDetect::No manifest file exists, will attempt to use Manifest.toml as default"
    );
    Some(project_root.join("Manifest.toml"))
}

fn find_highest_versioned_manifest(project_root: &Path) -> Option<PathBuf> {
    let Ok(entries) = fs::read_dir(project_root) else {
        return None;
    };

    let mut highest_version: Option<(Version, PathBuf)> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            // Check if the filename matches the pattern Manifest-v<version>.toml
            if let Some(stripped) = filename.strip_prefix("Manifest-v") {
                if let Some(version_str) = stripped.strip_suffix(".toml") {
                    // Try parsing the version, handling incomplete versions
                    if let Some(version) = parse_version_lenient(version_str) {
                        match &highest_version {
                            Some((current_version, _)) if &version > current_version => {
                                highest_version = Some((version, path));
                            }
                            None => {
                                highest_version = Some((version, path));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    highest_version.map(|(_, path)| path)
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
            "AutoVersionDetect::Manifest file `{}` not found while attempting to resolve Julia version.",
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

    // If requested version is higher than any known version, use nightly
    let required_version = Version::parse(&required).with_context(|| {
        format!(
            "Failed to parse Julia version `{}` from manifest.",
            required
        )
    })?;

    let max_known_version = max_available_version(versions_db)?;
    if let Some(max_version) = &max_known_version {
        if &required_version > max_version {
            return Ok("nightly".to_string());
        }
    } else {
        // No versions in database at all, use nightly
        return Ok("nightly".to_string());
    }

    Err(anyhow!(
        "Julia version `{}` requested by Project.toml/Manifest.toml is not available in the versions database.",
        required
    ))
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
    ) && (channel_valid || is_pr_channel(&resolved_channel))
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

    // Parse command line for +channel
    let mut channel_from_cmd_line: Option<String> = None;
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let first_arg = &args[1];

        if let Some(stripped) = first_arg.strip_prefix('+') {
            channel_from_cmd_line = Some(stripped.to_string());
        }
    }

    let (julia_channel_to_use, juliaup_channel_source) =
        if let Some(channel) = channel_from_cmd_line {
            (channel, JuliaupChannelSource::CmdLine)
        } else if let Ok(channel) = std::env::var("JULIAUP_CHANNEL") {
            (channel, JuliaupChannelSource::EnvVar)
        } else if let Ok(Some(channel)) = get_override_channel(&config_file) {
            (channel, JuliaupChannelSource::Override)
        } else if let Some(channel) = get_auto_channel(&args, &versiondb_data) {
            (channel, JuliaupChannelSource::Auto)
        } else if let Some(channel) = config_file.data.default.clone() {
            (channel, JuliaupChannelSource::Default)
        } else {
            return Err(anyhow!(
                "The Julia launcher failed to figure out which juliaup channel to use."
            ));
        };

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
    use serial_test::serial;
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
    #[serial]
    fn test_resolve_project_location_named_environment_dot() {
        // Test @. resolves to current directory's Project.toml
        let original_dir = std::env::current_dir().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");

        std::env::set_current_dir(temp_dir.path()).unwrap();
        let result = resolve_project_location("@.").unwrap();

        assert!(result.is_some());
        // Canonicalize both paths for comparison (handles symlinks on macOS)
        assert_eq!(
            result.unwrap().canonicalize().unwrap(),
            project_file.canonicalize().unwrap()
        );

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_determine_project_version_spec_from_project_flag() {
        // Test --project flag with explicit path
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.5");

        let args = vec![
            "julia".to_string(),
            format!("--project={}", temp_dir.path().display()),
            "-e".to_string(),
            "1+1".to_string(),
        ];

        let result = determine_project_version_spec(&args).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.10.5");
    }

    #[test]
    #[serial]
    fn test_determine_project_version_spec_from_project_flag_no_value() {
        // Test --project (without value) searches upward from current directory
        let original_dir = std::env::current_dir().unwrap();
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.11.0");

        std::env::set_current_dir(temp_dir.path()).unwrap();

        let args = vec![
            "julia".to_string(),
            "--project".to_string(),
            "-e".to_string(),
            "1+1".to_string(),
        ];

        let result = determine_project_version_spec(&args).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.11.0");

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    #[serial]
    fn test_determine_project_version_spec_from_env_var() {
        // Test JULIA_PROJECT environment variable
        let original_project = std::env::var("JULIA_PROJECT").ok();

        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.9.4");

        // Set JULIA_PROJECT environment variable
        std::env::set_var("JULIA_PROJECT", temp_dir.path());

        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result = determine_project_version_spec(&args).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.9.4");

        // Restore original
        match original_project {
            Some(val) => std::env::set_var("JULIA_PROJECT", val),
            None => std::env::remove_var("JULIA_PROJECT"),
        }
    }

    #[test]
    #[serial]
    fn test_determine_project_version_spec_from_env_var_empty_searches_upward() {
        // Test JULIA_PROJECT="" (empty) searches upward like @.
        let original_dir = std::env::current_dir().unwrap();
        let original_project = std::env::var("JULIA_PROJECT").ok();

        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.8.5");

        std::env::set_current_dir(temp_dir.path()).unwrap();
        std::env::set_var("JULIA_PROJECT", "");

        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result = determine_project_version_spec(&args).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.8.5");

        // Restore originals
        match original_project {
            Some(val) => std::env::set_var("JULIA_PROJECT", val),
            None => std::env::remove_var("JULIA_PROJECT"),
        }
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    #[serial]
    fn test_determine_project_version_spec_from_env_var_named_environment() {
        // Test JULIA_PROJECT=@. resolves to current directory
        let original_dir = std::env::current_dir().unwrap();
        let original_project = std::env::var("JULIA_PROJECT").ok();

        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.2");

        std::env::set_current_dir(temp_dir.path()).unwrap();
        std::env::set_var("JULIA_PROJECT", "@.");

        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result = determine_project_version_spec(&args).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.10.2");

        // Restore originals
        match original_project {
            Some(val) => std::env::set_var("JULIA_PROJECT", val),
            None => std::env::remove_var("JULIA_PROJECT"),
        }
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    #[serial]
    fn test_determine_project_version_spec_flag_overrides_env() {
        // Test that --project flag takes precedence over JULIA_PROJECT env var
        let original_project = std::env::var("JULIA_PROJECT").ok();

        let temp_dir1 = TempDir::new().unwrap();
        create_test_project(temp_dir1.path(), "name = \"Project1\"");
        create_manifest(temp_dir1.path(), "Manifest.toml", "1.9.0");

        let temp_dir2 = TempDir::new().unwrap();
        create_test_project(temp_dir2.path(), "name = \"Project2\"");
        create_manifest(temp_dir2.path(), "Manifest.toml", "1.10.0");

        // Set env var to temp_dir1
        std::env::set_var("JULIA_PROJECT", temp_dir1.path());

        // But use --project to point to temp_dir2
        let args = vec![
            "julia".to_string(),
            format!("--project={}", temp_dir2.path().display()),
            "-e".to_string(),
            "1+1".to_string(),
        ];

        let result = determine_project_version_spec(&args).unwrap();
        assert!(result.is_some());
        // Should use the version from temp_dir2 (flag), not temp_dir1 (env)
        assert_eq!(result.unwrap(), "1.10.0");

        // Restore original
        match original_project {
            Some(val) => std::env::set_var("JULIA_PROJECT", val),
            None => std::env::remove_var("JULIA_PROJECT"),
        }
    }

    #[test]
    fn test_determine_project_version_spec_no_project_specified() {
        // Test that None is returned when no project is specified
        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        // Make sure JULIA_PROJECT and JULIA_LOAD_PATH are not set
        std::env::remove_var("JULIA_PROJECT");
        std::env::remove_var("JULIA_LOAD_PATH");

        let result = determine_project_version_spec(&args).unwrap();
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_determine_project_version_spec_from_load_path() {
        // Test JULIA_LOAD_PATH environment variable
        // Save original JULIA_LOAD_PATH and JULIA_PROJECT
        let original_load_path = std::env::var("JULIA_LOAD_PATH").ok();
        let original_project = std::env::var("JULIA_PROJECT").ok();

        // Clear JULIA_PROJECT to ensure it doesn't interfere
        std::env::remove_var("JULIA_PROJECT");

        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.12.0");

        // Set JULIA_LOAD_PATH to include our test project
        std::env::set_var(
            "JULIA_LOAD_PATH",
            format!(
                "@{}{}{}@stdlib",
                LOAD_PATH_SEPARATOR,
                temp_dir.path().display(),
                LOAD_PATH_SEPARATOR
            ),
        );

        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result = determine_project_version_spec(&args).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.12.0");

        // Restore originals
        match original_load_path {
            Some(val) => std::env::set_var("JULIA_LOAD_PATH", val),
            None => std::env::remove_var("JULIA_LOAD_PATH"),
        }
        match original_project {
            Some(val) => std::env::set_var("JULIA_PROJECT", val),
            None => std::env::remove_var("JULIA_PROJECT"),
        }
    }

    #[test]
    #[serial]
    fn test_determine_project_version_spec_load_path_searches_first_valid() {
        // Test that JULIA_LOAD_PATH returns the first valid project
        // Save original JULIA_LOAD_PATH and JULIA_PROJECT
        let original_load_path = std::env::var("JULIA_LOAD_PATH").ok();
        let original_project = std::env::var("JULIA_PROJECT").ok();

        // Clear JULIA_PROJECT to ensure it doesn't interfere
        std::env::remove_var("JULIA_PROJECT");

        let temp_dir1 = TempDir::new().unwrap();
        create_test_project(temp_dir1.path(), "name = \"Project1\"");
        create_manifest(temp_dir1.path(), "Manifest.toml", "1.11.5");

        let temp_dir2 = TempDir::new().unwrap();
        create_test_project(temp_dir2.path(), "name = \"Project2\"");
        create_manifest(temp_dir2.path(), "Manifest.toml", "1.10.3");

        // Set JULIA_LOAD_PATH with temp_dir1 first
        std::env::set_var(
            "JULIA_LOAD_PATH",
            format!(
                "{}{}{}",
                temp_dir1.path().display(),
                LOAD_PATH_SEPARATOR,
                temp_dir2.path().display()
            ),
        );

        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result = determine_project_version_spec(&args).unwrap();
        assert!(result.is_some());
        // Should use version from temp_dir1 (first in LOAD_PATH)
        assert_eq!(result.unwrap(), "1.11.5");

        // Restore originals
        match original_load_path {
            Some(val) => std::env::set_var("JULIA_LOAD_PATH", val),
            None => std::env::remove_var("JULIA_LOAD_PATH"),
        }
        match original_project {
            Some(val) => std::env::set_var("JULIA_PROJECT", val),
            None => std::env::remove_var("JULIA_PROJECT"),
        }
    }

    #[test]
    #[serial]
    fn test_determine_project_version_spec_project_overrides_load_path() {
        // Test that JULIA_PROJECT takes precedence over JULIA_LOAD_PATH
        // Save originals
        let original_load_path = std::env::var("JULIA_LOAD_PATH").ok();
        let original_project = std::env::var("JULIA_PROJECT").ok();

        let temp_dir1 = TempDir::new().unwrap();
        create_test_project(temp_dir1.path(), "name = \"Project1\"");
        create_manifest(temp_dir1.path(), "Manifest.toml", "1.9.2");

        let temp_dir2 = TempDir::new().unwrap();
        create_test_project(temp_dir2.path(), "name = \"Project2\"");
        create_manifest(temp_dir2.path(), "Manifest.toml", "1.10.4");

        // Set JULIA_LOAD_PATH to temp_dir1
        std::env::set_var("JULIA_LOAD_PATH", temp_dir1.path().to_str().unwrap());
        // Set JULIA_PROJECT to temp_dir2 (should take precedence)
        std::env::set_var("JULIA_PROJECT", temp_dir2.path().to_str().unwrap());

        let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

        let result = determine_project_version_spec(&args).unwrap();
        assert!(result.is_some());
        // Should use version from temp_dir2 (JULIA_PROJECT), not temp_dir1 (JULIA_LOAD_PATH)
        assert_eq!(result.unwrap(), "1.10.4");

        // Restore originals
        match original_load_path {
            Some(val) => std::env::set_var("JULIA_LOAD_PATH", val),
            None => std::env::remove_var("JULIA_LOAD_PATH"),
        }
        match original_project {
            Some(val) => std::env::set_var("JULIA_PROJECT", val),
            None => std::env::remove_var("JULIA_PROJECT"),
        }
    }

    #[test]
    #[serial]
    fn test_determine_project_version_spec_relative_paths() {
        // Test that relative paths work like Julia (relative to current directory)
        // Similar to: JULIA_LOAD_PATH="Pkg.jl" julia --project=DataFrames.jl -e '...'
        let original_dir = std::env::current_dir().unwrap();
        let original_load_path = std::env::var("JULIA_LOAD_PATH").ok();
        let original_project = std::env::var("JULIA_PROJECT").ok();
        let parent_dir = TempDir::new().unwrap();

        // Clear JULIA_PROJECT to ensure it doesn't interfere
        std::env::remove_var("JULIA_PROJECT");

        // Create two project directories
        let pkg_dir = parent_dir.path().join("Pkg.jl");
        fs::create_dir(&pkg_dir).unwrap();
        create_test_project(&pkg_dir, "name = \"Pkg\"");
        create_manifest(&pkg_dir, "Manifest.toml", "1.9.0");

        let df_dir = parent_dir.path().join("DataFrames.jl");
        fs::create_dir(&df_dir).unwrap();
        create_test_project(&df_dir, "name = \"DataFrames\"");
        create_manifest(&df_dir, "Manifest.toml", "1.11.0");

        // Change to parent directory so relative paths work
        std::env::set_current_dir(parent_dir.path()).unwrap();

        // Set JULIA_LOAD_PATH to Pkg.jl
        std::env::set_var("JULIA_LOAD_PATH", "Pkg.jl");

        // Test 1: --project=DataFrames.jl should override JULIA_LOAD_PATH
        let args = vec![
            "julia".to_string(),
            "--project=DataFrames.jl".to_string(),
            "-e".to_string(),
            "1+1".to_string(),
        ];
        let result = determine_project_version_spec(&args).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "1.11.0");

        // Test 2: Without --project, should use JULIA_LOAD_PATH (Pkg.jl)
        let args2 = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];
        let result2 = determine_project_version_spec(&args2).unwrap();
        assert!(result2.is_some());
        assert_eq!(result2.unwrap(), "1.9.0");

        // Restore originals
        match original_load_path {
            Some(val) => std::env::set_var("JULIA_LOAD_PATH", val),
            None => std::env::remove_var("JULIA_LOAD_PATH"),
        }
        match original_project {
            Some(val) => std::env::set_var("JULIA_PROJECT", val),
            None => std::env::remove_var("JULIA_PROJECT"),
        }
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_resolve_path_to_project_direct_file() {
        // Test resolving a direct path to a project file
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");

        let result = resolve_path_to_project(&project_file).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), project_file);
    }

    #[test]
    fn test_resolve_path_to_project_directory() {
        // Test resolving a directory to its Project.toml
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");

        let result = resolve_path_to_project(temp_dir.path()).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), project_file);
    }

    #[test]
    fn test_resolve_path_to_project_search_upward() {
        // Test searching upward for Project.toml
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");

        // Create a subdirectory
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();

        let result = resolve_path_to_project(&subdir).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), project_file);
    }

    #[test]
    fn test_resolve_path_to_project_julia_project_precedence() {
        // Test that JuliaProject.toml takes precedence over Project.toml
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "name = \"TestProject\"");
        let julia_project_file = temp_dir.path().join("JuliaProject.toml");
        fs::write(&julia_project_file, "name = \"JuliaTestProject\"").unwrap();

        let result = resolve_path_to_project(temp_dir.path()).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), julia_project_file);
    }

    #[test]
    fn test_resolve_path_to_project_not_found() {
        // Test when no project file exists
        let temp_dir = TempDir::new().unwrap();
        let result = resolve_path_to_project(temp_dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_determine_manifest_path_default() {
        // Test default Manifest.toml detection
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.0");

        let project_toml: Value = toml::from_str("name = \"TestProject\"").unwrap();
        let result = determine_manifest_path(&project_file, &project_toml);

        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "Manifest.toml");
    }

    #[test]
    fn test_determine_manifest_path_julia_manifest_precedence() {
        // Test that JuliaManifest.toml takes precedence over Manifest.toml
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.10.0");
        create_manifest(temp_dir.path(), "JuliaManifest.toml", "1.11.0");

        let project_toml: Value = toml::from_str("name = \"TestProject\"").unwrap();
        let result = determine_manifest_path(&project_file, &project_toml);

        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "JuliaManifest.toml");
    }

    #[test]
    fn test_determine_manifest_path_versioned_manifest() {
        // Test versioned manifest detection
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");

        let project_toml: Value = toml::from_str("name = \"TestProject\"").unwrap();
        let result = determine_manifest_path(&project_file, &project_toml);

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

        let project_toml: Value = toml::from_str("name = \"TestProject\"").unwrap();
        let result = determine_manifest_path(&project_file, &project_toml);

        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "Manifest-v1.12.toml");
    }

    #[test]
    fn test_determine_manifest_path_standard_over_versioned() {
        // Test that standard Manifest.toml takes precedence over versioned manifests
        let temp_dir = TempDir::new().unwrap();
        let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
        create_manifest(temp_dir.path(), "Manifest.toml", "1.13.0");
        create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");
        create_manifest(temp_dir.path(), "Manifest-v1.12.toml", "1.12.0");

        let project_toml: Value = toml::from_str("name = \"TestProject\"").unwrap();
        let result = determine_manifest_path(&project_file, &project_toml);

        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "Manifest.toml");
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

        let project_toml: Value = toml::from_str(indoc! {r#"
            name = "TestProject"
            manifest = "custom/Manifest.toml"
        "#})
        .unwrap();
        let result = determine_manifest_path(&project_file, &project_toml);

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
}
