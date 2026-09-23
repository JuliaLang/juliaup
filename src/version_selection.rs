use anyhow::{anyhow, bail, Context, Result};
use semver::Version;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use toml::Table;

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

        match load_path_expand_impl(entry, current_dir, depot_path)? {
            Some(project_file) => {
                if project_file_manifest_path(&project_file)?.is_some() {
                    log::debug!("Found valid project in JULIA_LOAD_PATH entry: {}", entry);
                    return Ok(Some(project_file));
                }
            }
            None => continue, // Entry resolved to None (e.g., @stdlib, @), try next
        }
    }

    log::debug!("No valid project with manifest found in JULIA_LOAD_PATH");
    Ok(None)
}

const PROJECT_FLAGS: &[&str] = &["--project", "--projec", "--proje", "--proj"];

fn match_project_flag(arg: &str) -> Option<Option<String>> {
    PROJECT_FLAGS.iter().find_map(|flag| {
        if arg == *flag {
            // --proj / --proje / --projec / --project
            Some(None)
        } else {
            // --proj=val / --proje=val / --projec=val / --project=val
            arg.strip_prefix(flag)
                .and_then(|rest| rest.strip_prefix('='))
                .map(|v| Some(v.to_string()))
        }
    })
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
    if let Some(short) = opt.strip_prefix('-').filter(|s| !s.starts_with('-')) {
        let mut chars = short.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        if chars.next().is_some() {
            // The argument is attached (e.g. "-t4", "-O3", "-e1+1")
            return false;
        }
        // no_argument: v, h, q, i
        // optional_argument: O, g
        return !matches!(first, 'v' | 'h' | 'q' | 'i' | 'O' | 'g');
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

        if let Some(spec) = match_project_flag(arg) {
            project_cli = Some(spec);
        } else if julia_option_requires_arg(arg) {
            // This option consumes the next token as its argument
            // Skip it to avoid treating the argument as a flag
            args_iter.next();
        }
    }

    // Determine project spec
    let maybe_project = if let Some(spec) = project_cli {
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

    // Expand the project spec using load_path_expand
    if let Some(project) = maybe_project {
        load_path_expand_impl(&project, current_dir, depot_path)
    } else {
        Ok(None)
    }
}

/// Read and parse a TOML file, with error messages that name the file.
fn read_toml_file(path: &Path) -> Result<Table> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read `{}`", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("Failed to parse `{}` as TOML", path.display()))
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Find the project file of the workspace that directly includes `project_file`
/// (Julia's `base_project`).
/// https://github.com/JuliaLang/julia/blob/v1.12.0/base/loading.jl
pub fn base_project(project_file: &Path) -> Result<Option<PathBuf>> {
    let Some(project_dir) = project_file.parent() else {
        return Ok(None);
    };
    let home_dir = dirs::home_dir();
    // Only stop at the home directory boundary if we started under it
    let started_in_home = home_dir
        .as_ref()
        .is_some_and(|home| project_dir.starts_with(home));

    let mut current_dir = project_dir;
    while let Some(parent_dir) = current_dir.parent() {
        if started_in_home
            && !home_dir
                .as_ref()
                .is_some_and(|home| parent_dir.starts_with(home))
        {
            return Ok(None);
        }

        if let Some(base_project_file) = find_project_file_in_dir(parent_dir) {
            let parsed = read_toml_file(&base_project_file)?;
            if let Some(projects) = parsed
                .get("workspace")
                .and_then(|w| w.get("projects"))
                .and_then(|p| p.as_array())
            {
                for project in projects.iter().filter_map(|p| p.as_str()) {
                    let project_path = parent_dir.join(project);
                    if project_path.is_dir() && same_path(&project_path, project_dir) {
                        return Ok(Some(base_project_file));
                    }
                }
            }
        }

        current_dir = parent_dir;
    }

    Ok(None)
}

/// Find the root project file of the workspace `project_file` belongs to, or
/// `project_file` itself if it is not part of a workspace.
pub fn find_root_base_project(project_file: &Path) -> Result<PathBuf> {
    let mut current = project_file.to_path_buf();
    while let Some(base) = base_project(&current)? {
        current = base;
    }
    Ok(current)
}

/// Collect the project files of all projects in the workspace rooted at `root`
/// (Pkg's `collect_workspace`).
fn collect_workspace(root: &Path, projects: &mut Vec<PathBuf>) -> Result<()> {
    if projects.iter().any(|p| same_path(p, root)) {
        return Ok(());
    }
    projects.push(root.to_path_buf());

    let parsed = read_toml_file(root)?;
    let Some(members) = parsed
        .get("workspace")
        .and_then(|w| w.get("projects"))
        .and_then(|p| p.as_array())
    else {
        return Ok(());
    };
    let root_dir = root.parent().unwrap_or(Path::new("."));
    for member in members.iter().filter_map(|p| p.as_str()) {
        if let Some(member_project) = find_project_file_in_dir(&root_dir.join(member)) {
            collect_workspace(&member_project, projects)?;
        }
    }
    Ok(())
}

/// A manifest file found for a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundManifest {
    pub path: PathBuf,
    /// Whether this is a `JuliaManifest-vX.Y.toml`/`Manifest-vX.Y.toml` file.
    pub versioned: bool,
}

/// Find the manifest file that a Julia of the given `major.minor` version would
/// load for `project_file` (Julia's `project_file_manifest_path`).
/// https://github.com/JuliaLang/julia/blob/v1.12.0/base/loading.jl
///
/// Julia's priority order is:
/// 1. The manifest of the workspace the project belongs to
/// 2. The explicit `manifest` field in the project file, if that file exists
/// 3. `JuliaManifest-v{major}.{minor}.toml`, then `Manifest-v{major}.{minor}.toml`
/// 4. `JuliaManifest.toml`, then `Manifest.toml`
///
/// When `julia_minor` is `None`, versioned manifests (rule 3) are not considered.
pub fn manifest_for_julia_version(
    project_file: &Path,
    julia_minor: Option<(u64, u64)>,
) -> Result<Option<FoundManifest>> {
    if !project_file.is_file() {
        return Ok(None);
    }
    let Some(dir) = project_file.parent() else {
        return Ok(None);
    };

    let parsed_project = read_toml_file(project_file)?;

    if let Some(base) = base_project(project_file)? {
        if let Some(manifest) = manifest_for_julia_version(&base, julia_minor)? {
            return Ok(Some(manifest));
        }
    }

    if let Some(explicit_manifest) = parsed_project.get("manifest").and_then(|v| v.as_str()) {
        let manifest_file = dir.join(explicit_manifest);
        if manifest_file.is_file() {
            return Ok(Some(FoundManifest {
                path: manifest_file,
                versioned: false,
            }));
        }
    }

    if let Some((major, minor)) = julia_minor {
        let versioned_names = [
            format!("JuliaManifest-v{}.{}.toml", major, minor),
            format!("Manifest-v{}.{}.toml", major, minor),
        ];
        if let Some(path) = versioned_names
            .iter()
            .map(|name| dir.join(name))
            .find(|path| path.is_file())
        {
            return Ok(Some(FoundManifest {
                path,
                versioned: true,
            }));
        }
    }

    Ok(
        find_named_file(dir, MANIFEST_NAMES).map(|path| FoundManifest {
            path,
            versioned: false,
        }),
    )
}

/// Find the manifest file used to select a Julia version for a project: the
/// manifest Julia would load, ignoring versioned manifests.
pub fn project_file_manifest_path(project_file: &Path) -> Result<Option<PathBuf>> {
    Ok(manifest_for_julia_version(project_file, None)?.map(|m| m.path))
}

/// A `julia` entry in the `[compat]` section of a project file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JuliaCompatEntry {
    pub project_file: PathBuf,
    pub spec: String,
}

/// Everything the launcher needs to know about the active project.
#[derive(Debug, Clone)]
pub struct ProjectContext {
    /// The active project file.
    pub project_file: PathBuf,
    /// The root project file of the workspace the project belongs to, if any.
    pub workspace_root: Option<PathBuf>,
    /// The manifest that Julia will load, if any. When the base manifest records
    /// a Julia version and a versioned manifest for the same minor version
    /// exists, this is the versioned manifest.
    pub manifest_file: Option<PathBuf>,
    /// Whether `manifest_file` is a versioned manifest (`Manifest-vX.Y.toml`).
    pub manifest_is_versioned: bool,
    /// The `julia_version` recorded in `manifest_file`.
    pub julia_version: Option<String>,
    /// The `[compat] julia` entries of all projects in the workspace.
    pub julia_compat: Vec<JuliaCompatEntry>,
    /// Whether the active project is a package (has a `name` and a `uuid`).
    pub is_package: bool,
    /// Whether any project in the workspace has dependencies.
    pub has_deps: bool,
}

impl ProjectContext {
    /// The directory of the active project.
    pub fn project_dir(&self) -> &Path {
        self.project_file.parent().unwrap_or(Path::new("."))
    }

    /// Whether the manifest is located somewhere other than next to the
    /// active project file (workspace root, or an explicit `manifest` field).
    pub fn manifest_is_elsewhere(&self) -> bool {
        match &self.manifest_file {
            Some(manifest) => manifest.parent() != Some(self.project_dir()),
            None => false,
        }
    }
}

/// Determines the active project and the Julia version recorded in its manifest,
/// based on the arguments to julia and environment variables.
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
/// Returns `None` if no project is active or the project file does not exist.
/// Returns an error if a project file or manifest exists but can't be read or
/// understood.
pub fn determine_project_context(args: &[String]) -> Result<Option<ProjectContext>> {
    determine_project_context_impl(
        args,
        std::env::var("JULIA_PROJECT").ok(),
        std::env::var("JULIA_LOAD_PATH").ok(),
        &std::env::current_dir().with_context(|| "Failed to determine current directory.")?,
    )
}

pub fn determine_project_context_impl(
    args: &[String],
    julia_project: Option<String>,
    julia_load_path: Option<String>,
    current_dir: &Path,
) -> Result<Option<ProjectContext>> {
    let depot_path = std::env::var_os("JULIA_DEPOT_PATH");

    // Resolve project file (in priority order)
    // 1. --project flag or JULIA_PROJECT env
    let mut maybe_project_file = init_active_project_impl(
        args,
        current_dir,
        julia_project.as_deref(),
        depot_path.as_deref(),
    )?;
    // 2. Fallback to JULIA_LOAD_PATH
    if maybe_project_file.is_none() {
        if let Some(load_path) = &julia_load_path {
            maybe_project_file =
                find_project_from_load_path(load_path, current_dir, depot_path.as_deref())?;
        }
    }

    // If no project was found, stop here
    let Some(project_file) = maybe_project_file else {
        log::debug!("No project specification found");
        return Ok(None);
    };

    if !project_file.is_file() {
        log::debug!("Project file {} does not exist", project_file.display());
        return Ok(None);
    }

    project_context_from_project_file(project_file).map(Some)
}

/// Build the `ProjectContext` for an existing project file.
pub fn project_context_from_project_file(project_file: PathBuf) -> Result<ProjectContext> {
    log::debug!("Using project file: {}", project_file.display());

    let parsed_project = read_toml_file(&project_file)?;
    let is_package = parsed_project.contains_key("name") && parsed_project.contains_key("uuid");

    let root = find_root_base_project(&project_file)?;
    let workspace_root = if same_path(&root, &project_file) {
        None
    } else {
        Some(root.clone())
    };

    let mut workspace_projects = Vec::new();
    collect_workspace(&root, &mut workspace_projects)?;
    if !workspace_projects
        .iter()
        .any(|p| same_path(p, &project_file))
    {
        workspace_projects.push(project_file.clone());
    }

    let mut julia_compat = Vec::new();
    let mut has_deps = false;
    for member in &workspace_projects {
        let parsed = read_toml_file(member)?;
        if parsed
            .get("deps")
            .and_then(|d| d.as_table())
            .is_some_and(|d| !d.is_empty())
        {
            has_deps = true;
        }
        if let Some(compat) = parsed.get("compat").and_then(|c| c.get("julia")) {
            let spec = compat.as_str().ok_or_else(|| {
                anyhow!(
                    "The `julia` entry in the `[compat]` section of `{}` is not a string.",
                    member.display()
                )
            })?;
            julia_compat.push(JuliaCompatEntry {
                project_file: member.clone(),
                spec: spec.to_string(),
            });
        }
    }

    let mut context = ProjectContext {
        project_file,
        workspace_root,
        manifest_file: None,
        manifest_is_versioned: false,
        julia_version: None,
        julia_compat,
        is_package,
        has_deps,
    };

    // The base manifest (ignoring versioned manifests) determines the Julia
    // minor version.
    let Some(base) = manifest_for_julia_version(&context.project_file, None)? else {
        log::debug!("No manifest file found for project");
        return Ok(context);
    };
    log::debug!("Detected manifest file: {}", base.path.display());

    let base_version = read_manifest_julia_version(&base.path)?;
    context.manifest_file = Some(base.path.clone());
    let Some(base_version) = base_version else {
        log::debug!("Manifest file exists but does not contain julia_version field");
        return Ok(context);
    };
    let parsed_base_version = parse_manifest_julia_version(&base_version, &base.path)?;
    context.julia_version = Some(base_version.clone());

    // A Julia of that minor version loads a same-minor versioned manifest if one
    // exists, so the exact version is taken from that file.
    let effective = manifest_for_julia_version(
        &context.project_file,
        Some((parsed_base_version.major, parsed_base_version.minor)),
    )?;
    if let Some(effective) = effective.filter(|m| m.versioned) {
        log::debug!(
            "Detected versioned manifest file: {}",
            effective.path.display()
        );
        let versioned_version = read_manifest_julia_version(&effective.path)?.ok_or_else(|| {
            anyhow!(
                "The manifest `{}` does not record a `julia_version`. Julia {}.{} loads this manifest instead of `{}`.",
                effective.path.display(),
                parsed_base_version.major,
                parsed_base_version.minor,
                base.path.display()
            )
        })?;
        let parsed = parse_manifest_julia_version(&versioned_version, &effective.path)?;
        if (parsed.major, parsed.minor) != (parsed_base_version.major, parsed_base_version.minor) {
            bail!(
                "The manifest `{}` records Julia {}, but Julia {}.{} loads it instead of `{}` (which records Julia {}).",
                effective.path.display(),
                versioned_version,
                parsed_base_version.major,
                parsed_base_version.minor,
                base.path.display(),
                base_version
            );
        }
        context.manifest_file = Some(effective.path);
        context.manifest_is_versioned = true;
        context.julia_version = Some(versioned_version);
    }

    Ok(context)
}

/// Determines the Julia version recorded in the active project's manifest.
/// See [`determine_project_context_impl`].
pub fn determine_project_version_spec_impl(
    args: &[String],
    julia_project: Option<String>,
    julia_load_path: Option<String>,
    current_dir: &Path,
) -> Result<Option<String>> {
    Ok(
        determine_project_context_impl(args, julia_project, julia_load_path, current_dir)?
            .and_then(|context| context.julia_version),
    )
}

fn parse_manifest_julia_version(version: &str, manifest: &Path) -> Result<Version> {
    Version::parse(version).with_context(|| {
        format!(
            "Failed to parse the Julia version `{}` recorded in `{}`.",
            version,
            manifest.display()
        )
    })
}

pub fn read_manifest_julia_version(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        log::debug!(
            "Manifest file `{}` not found while attempting to resolve Julia version.",
            path.display()
        );
        return Ok(None);
    }

    let manifest = read_toml_file(path)?;

    match manifest.get("julia_version") {
        None => Ok(None),
        Some(v) => v.as_str().map(|s| Some(s.to_string())).ok_or_else(|| {
            anyhow!(
                "The `julia_version` entry in `{}` is not a string.",
                path.display()
            )
        }),
    }
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
    pub fn has_channel(&self, version: &str) -> bool {
        self.available_channels.contains_key(version)
    }

    /// All stable (non-prerelease) Julia versions known to the versions db.
    pub fn stable_versions(&self) -> Vec<Version> {
        let mut versions: Vec<Version> = self
            .available_channels
            .values()
            .filter_map(|channel| parse_db_version(&channel.version).ok())
            .filter(|version| version.pre.is_empty())
            .collect();
        versions.sort();
        versions.dedup();
        versions
    }
}

/// A release version of Julia that the versions db does not know about.
#[derive(Debug, thiserror::Error)]
#[error("Julia {version} is not a known Julia release.")]
pub struct UnknownJuliaVersion {
    pub version: String,
}

/// Map the Julia version recorded in a manifest to a Juliaup channel.
///
/// Released versions map to the channel of the same name. Prerelease versions
/// (e.g. `1.12.1-DEV`, `1.13.0-rc1`) without a channel of their own map to a
/// nightly channel. A release version that the versions db does not know about
/// returns an [`UnknownJuliaVersion`] error. With `announce`, the mapping of a
/// prerelease version to a nightly channel is reported on stderr.
pub fn resolve_auto_channel(
    required: &str,
    versions_db: &JuliaupVersionDB,
    announce: bool,
) -> Result<String> {
    // Check if exact version is available
    if versions_db.has_channel(required) {
        return Ok(required.to_string());
    }

    let required_version = Version::parse(required).with_context(|| {
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

        if versions_db.has_channel(&versioned_nightly) {
            if announce {
                print_juliaup_style(
                    "Info",
                    &format!(
                        "Manifest specifies prerelease Julia {}. Using {} channel.",
                        required, versioned_nightly
                    ),
                    JuliaupMessageType::Progress,
                );
            }
            return Ok(versioned_nightly);
        }

        // Fall back to main nightly channel
        if announce {
            print_juliaup_style(
                "Info",
                &format!(
                    "Manifest specifies prerelease Julia {}. Using nightly channel.",
                    required
                ),
                JuliaupMessageType::Progress,
            );
        }
        return Ok("nightly".to_string());
    }

    Err(UnknownJuliaVersion {
        version: required.to_string(),
    }
    .into())
}
