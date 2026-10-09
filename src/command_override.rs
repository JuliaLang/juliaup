use std::{
    env::current_dir,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use cli_table::{
    format::{Border, HorizontalLine, Separator},
    print_stdout, ColorChoice, Table, WithTitle,
};
use itertools::Itertools;
use path_absolutize::Absolutize;

use crate::{
    config_file::{load_config_db, load_mut_config_db, save_config_db, JuliaupOverride},
    global_paths::GlobalPaths,
};

#[derive(Table)]
struct OverrideRow {
    #[table(title = "Path")]
    path: String,
    #[table(title = "Channel")]
    channel: String,
}

pub fn run_command_override_status(paths: &GlobalPaths) -> Result<()> {
    let config_file = load_config_db(paths, None)
        .with_context(|| "`override status` command failed to load configuration file.")?;

    let rows_in_table: Vec<_> = config_file
        .data
        .overrides
        .iter()
        .sorted_by_key(|i| i.path.to_string())
        .map(|i| -> OverrideRow {
            OverrideRow {
                path: dunce::simplified(&PathBuf::from(&i.path))
                    .to_string_lossy()
                    .to_string(),
                channel: i.channel.to_string(),
            }
        })
        .collect();

    print_stdout(
        rows_in_table
            .with_title()
            .color_choice(ColorChoice::Never)
            .border(Border::builder().build())
            .separator(
                Separator::builder()
                    .title(Some(HorizontalLine::new('1', '2', '3', '-')))
                    .build(),
            ),
    )?;

    Ok(())
}

pub fn run_command_override_set(
    paths: &GlobalPaths,
    channel: String,
    path: Option<String>,
) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`override set` command failed to load configuration data.")?;

    if !config_file.data.installed_channels.contains_key(&channel) {
        bail!(
            "'{}' is not installed. Please run `juliaup add {}` to install channel or version.",
            channel,
            channel
        );
    }

    let path = match path {
        Some(path) => PathBuf::from(path),
        None => current_dir()?,
    }
    .canonicalize()?;

    if let Some(i) = config_file
        .data
        .overrides
        .iter_mut()
        .find(|i| i.path == path.to_string_lossy())
    {
        if i.channel == channel {
            eprintln!(
                "Override already set to '{}' for '{}'.",
                channel,
                path.to_string_lossy()
            );
            return Ok(());
        } else {
            eprintln!(
                "Override changed from '{}' to '{}' for '{}'.",
                i.channel,
                channel,
                path.to_string_lossy()
            );
            i.channel = channel.clone();

            save_config_db(&mut config_file, paths).with_context(|| {
                format!(
                    "Failed to save configuration file from `override set` command at `{}`.",
                    paths.juliaupconfig.display()
                )
            })?;
            return Ok(());
        }
    }

    config_file.data.overrides.push(JuliaupOverride {
        path: path.to_string_lossy().to_string(),
        channel: channel.clone(),
    });
    eprintln!(
        "Override set to '{}' for '{}'.",
        channel,
        path.to_string_lossy()
    );

    save_config_db(&mut config_file, paths).with_context(|| {
        format!(
            "Failed to save configuration file from `override set` command at `{}`.",
            paths.juliaupconfig.display()
        )
    })?;

    Ok(())
}

/// Like `Path::canonicalize`, but also works for a path that no longer exists
/// (https://github.com/JuliaLang/juliaup/issues/1196). The deepest ancestor
/// that still exists is canonicalized and the missing components are appended,
/// so a deleted directory maps to the same path that `override set` stored
/// while it existed, even if one of its parents is a symlink (e.g. `/var` ->
/// `/private/var` on macOS).
fn canonicalize_allow_missing(path: &Path) -> Result<PathBuf> {
    let path = path
        .absolutize()
        .with_context(|| format!("Failed to make `{}` absolute.", path.display()))?;

    for ancestor in path.ancestors() {
        if let Ok(canonical) = ancestor.canonicalize() {
            let missing = path.strip_prefix(ancestor)?;
            return Ok(if missing.as_os_str().is_empty() {
                canonical
            } else {
                canonical.join(missing)
            });
        }
    }

    bail!("Failed to resolve `{}`.", path.display())
}

pub fn run_command_override_unset(
    paths: &GlobalPaths,
    nonexistent: bool,
    path: Option<String>,
) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`override unset` command failed to load configuration data.")?;

    if nonexistent {
        config_file
            .data
            .overrides
            .retain(|x| Path::new(&x.path).is_dir());
    } else {
        let path = canonicalize_allow_missing(&match path {
            Some(path) => PathBuf::from(path),
            None => current_dir()?,
        })?;

        // First remove any duplicates
        config_file
            .data
            .overrides
            .retain(|x| Path::new(&x.path) != path);
    }

    save_config_db(&mut config_file, paths).with_context(|| {
        format!(
            "Failed to save configuration file from `override unset` command at `{}`.",
            paths.juliaupconfig.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::canonicalize_allow_missing;

    #[test]
    fn canonicalize_allow_missing_matches_canonicalize_for_existing_paths() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            canonicalize_allow_missing(dir.path()).unwrap(),
            dir.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn canonicalize_allow_missing_appends_missing_components() {
        let dir = tempfile::tempdir().unwrap();
        let canonical_dir = dir.path().canonicalize().unwrap();
        assert_eq!(
            canonicalize_allow_missing(&dir.path().join("gone")).unwrap(),
            canonical_dir.join("gone")
        );
        assert_eq!(
            canonicalize_allow_missing(&dir.path().join("a").join("b")).unwrap(),
            canonical_dir.join("a").join("b")
        );
    }
}
