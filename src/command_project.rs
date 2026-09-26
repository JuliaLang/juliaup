//! Internal commands the `julia` launcher runs to change a project: upgrading
//! the Julia version recorded in its manifest, and pinning its Julia version in
//! the `[compat]` section.

use anyhow::{anyhow, bail, Context, Result};
use semver::Version;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::command_add::run_command_add;
use crate::config_file::{load_config_db, JuliaupConfigVersion};
use crate::global_paths::GlobalPaths;
use crate::utils::{print_juliaup_style, resolve_julia_binary_path, JuliaupMessageType};
use crate::version_selection::{project_context_from_project_file, ProjectContext};
use crate::versions_file::load_versions_db;

fn version_binary_path(
    version_info: &JuliaupConfigVersion,
    paths: &GlobalPaths,
) -> Result<PathBuf> {
    let config_dir = paths
        .juliaupconfig
        .parent()
        .ok_or_else(|| anyhow!("Invalid juliaup configuration path."))?;
    match &version_info.binary_path {
        Some(binary_path) => Ok(config_dir.join(binary_path)),
        None => resolve_julia_binary_path(&config_dir.join(&version_info.path)),
    }
}

/// The Julia binary for a Julia version, installing the version if needed.
fn julia_binary_for_version(version: &str, paths: &GlobalPaths) -> Result<PathBuf> {
    let find = || -> Result<Option<PathBuf>> {
        let versions_db = load_versions_db(paths)?;
        let Some(full_version) = versions_db
            .available_channels
            .get(version)
            .map(|c| c.version.clone())
        else {
            return Ok(None);
        };
        let config_file = load_config_db(paths, None)?;
        config_file
            .data
            .installed_versions
            .get(&full_version)
            .map(|info| version_binary_path(info, paths))
            .transpose()
    };

    if let Some(path) = find()? {
        return Ok(path);
    }

    run_command_add(version, paths)?;

    find()?.ok_or_else(|| anyhow!("Julia {} was installed but could not be found.", version))
}

/// The contents of all manifest files that running Pkg for the project might
/// change, so that they can be restored if something goes wrong.
struct ManifestSnapshot {
    files: BTreeMap<PathBuf, Option<Vec<u8>>>,
}

impl ManifestSnapshot {
    fn take(context: &ProjectContext) -> Result<Self> {
        let manifest_dir = context
            .manifest_file
            .as_deref()
            .and_then(|m| m.parent())
            .unwrap_or_else(|| context.project_dir())
            .to_path_buf();

        let mut candidates: Vec<PathBuf> = context.manifest_file.iter().cloned().collect();
        for dir in [manifest_dir.as_path(), context.project_dir()] {
            for entry in fs::read_dir(dir)
                .with_context(|| format!("Failed to read `{}`", dir.display()))?
                .flatten()
            {
                let name = entry.file_name().to_string_lossy().to_string();
                if (name.starts_with("Manifest") || name.starts_with("JuliaManifest"))
                    && name.ends_with(".toml")
                {
                    candidates.push(entry.path());
                }
            }
            // A new manifest may be created with the default names
            candidates.push(dir.join("Manifest.toml"));
        }

        let mut files = BTreeMap::new();
        for path in candidates {
            let content = if path.is_file() {
                Some(
                    fs::read(&path)
                        .with_context(|| format!("Failed to read `{}`", path.display()))?,
                )
            } else {
                None
            };
            files.insert(path, content);
        }
        Ok(ManifestSnapshot { files })
    }

    fn restore(&self) -> Result<()> {
        for (path, content) in &self.files {
            match content {
                Some(content) => {
                    if fs::read(path).ok().as_deref() != Some(content.as_slice()) {
                        fs::write(path, content)
                            .with_context(|| format!("Failed to restore `{}`", path.display()))?;
                    }
                }
                None => {
                    if path.is_file() {
                        fs::remove_file(path)
                            .with_context(|| format!("Failed to remove `{}`", path.display()))?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Run a Pkg command for the project with the given Julia binary. Julia's
/// output goes to stderr.
fn run_pkg(julia: &Path, context: &ProjectContext, code: &str) -> Result<bool> {
    let status = std::process::Command::new(julia)
        .arg(format!("--project={}", context.project_dir().display()))
        .args(["--startup-file=no", "--history-file=no", "-e", code])
        .stdin(std::process::Stdio::null())
        .stdout(std::io::stderr())
        .status()
        .with_context(|| format!("Failed to start Julia at `{}`.", julia.display()))?;
    Ok(status.success())
}

fn display_path(path: &Path) -> String {
    dunce::simplified(path).display().to_string()
}

/// Re-resolve the project with Julia `version`, so that its manifest records
/// that version. If that fails, all manifest files are restored.
pub fn run_command_project_upgrade(
    project_file: &str,
    version: &str,
    paths: &GlobalPaths,
) -> Result<()> {
    let target =
        Version::parse(version).with_context(|| format!("Invalid Julia version `{}`.", version))?;
    let context = project_context_from_project_file(PathBuf::from(project_file))?;
    let old_version = context
        .julia_version
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    let julia = julia_binary_for_version(version, paths)?;

    print_juliaup_style(
        "Upgrading",
        &format!(
            "project {} to Julia {}",
            display_path(context.project_dir()),
            version
        ),
        JuliaupMessageType::Progress,
    );

    let snapshot = ManifestSnapshot::take(&context)?;

    let succeeded = run_pkg(
        &julia,
        &context,
        "import Pkg; Pkg.resolve(); Pkg.instantiate()",
    )?;

    // Check that the manifest Julia will now load records the new version
    let updated = if succeeded {
        let updated = project_context_from_project_file(context.project_file.clone())?;
        let recorded = updated
            .julia_version
            .as_deref()
            .and_then(|v| Version::parse(v).ok());
        if recorded.as_ref() == Some(&target) {
            Some(updated)
        } else {
            None
        }
    } else {
        None
    };

    let Some(updated) = updated else {
        snapshot.restore()?;
        bail!(
            "Upgrading the project {} to Julia {} failed, the manifest was left unchanged.\n\
             This usually means that one of the project's dependencies does not support Julia {} yet. \
             To update the dependencies as well, start Julia {} with the project and run `Pkg.update()`.",
            display_path(context.project_dir()),
            version,
            version,
            version
        );
    };

    print_juliaup_style(
        "Updated",
        &format!(
            "{}: julia_version {} → {}",
            display_path(updated.manifest_file.as_deref().unwrap_or(Path::new(""))),
            old_version,
            version
        ),
        JuliaupMessageType::Success,
    );

    Ok(())
}

/// Set `julia = "=<version>"` in the `[compat]` section of the project file,
/// and re-resolve the project so that its manifest stays current.
pub fn run_command_project_pin(
    project_file: &str,
    version: &str,
    paths: &GlobalPaths,
) -> Result<()> {
    Version::parse(version).with_context(|| format!("Invalid Julia version `{}`.", version))?;
    let project_file = PathBuf::from(project_file);
    let context = project_context_from_project_file(project_file.clone())?;

    if context.is_package {
        bail!(
            "`{}` is a package; its Julia compat is not changed, because that would restrict the users of the package.",
            display_path(&project_file)
        );
    }

    let julia = julia_binary_for_version(version, paths)?;

    let original = fs::read_to_string(&project_file)
        .with_context(|| format!("Failed to read `{}`", project_file.display()))?;
    let mut doc: toml_edit::DocumentMut = original
        .parse()
        .with_context(|| format!("Failed to parse `{}` as TOML", project_file.display()))?;
    if !doc.contains_key("compat") {
        doc["compat"] = toml_edit::table();
    }
    let compat = doc["compat"].as_table_mut().ok_or_else(|| {
        anyhow!(
            "The `compat` entry in `{}` is not a table.",
            project_file.display()
        )
    })?;
    let pinned = format!("={}", version);
    compat["julia"] = toml_edit::value(pinned.clone());

    let snapshot = ManifestSnapshot::take(&context)?;
    fs::write(&project_file, doc.to_string())
        .with_context(|| format!("Failed to write `{}`", project_file.display()))?;

    // Update the project hash recorded in the manifest
    if !run_pkg(&julia, &context, "import Pkg; Pkg.resolve()")? {
        fs::write(&project_file, original)
            .with_context(|| format!("Failed to restore `{}`", project_file.display()))?;
        snapshot.restore()?;
        bail!(
            "Pinning the project {} to Julia {} failed, the project was left unchanged.",
            display_path(context.project_dir()),
            version
        );
    }

    print_juliaup_style(
        "Pinned",
        &format!(
            "project {} to Julia {} (compat julia = \"{}\" in {})",
            display_path(context.project_dir()),
            version,
            pinned,
            project_file
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        ),
        JuliaupMessageType::Success,
    );

    Ok(())
}
