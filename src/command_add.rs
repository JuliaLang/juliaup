use std::path::PathBuf;

use crate::config_file::{load_mut_config_db, save_config_db, JuliaupConfigChannel, load_config_db, JuliaupConfigVersion};
use crate::global_paths::GlobalPaths;
#[cfg(not(windows))]
use crate::operations::create_symlink;
use crate::operations::{identify_nightly, install_nightly, download_version_to_temp_folder, update_version_db};
use crate::versions_file::load_versions_db;
use anyhow::{anyhow, Context, Result};

pub fn run_command_add(channel: &str, paths: &GlobalPaths) -> Result<()> {
    if channel == "nightly" || channel.starts_with("nightly~") {
        return add_nightly(channel, paths);
    }

    update_version_db(paths).with_context(|| "Failed to update versions db.")?;
    let version_db =
        load_versions_db(paths).with_context(|| "`add` command failed to load versions db.")?;

    let required_version = &version_db
        .available_channels
        .get(channel)
        .ok_or_else(|| {
            anyhow!(
                "'{}' is not a valid Julia version or channel name.",
                &channel
            )
        })?
        .version;

    let we_need_to_download: bool;

    {
        let config_file = load_config_db(paths)
            .with_context(|| "`add` command failed to load configuration data.")?;

        if config_file.data.installed_channels.contains_key(channel) {
            bail!("'{}' is already installed.", &channel);
        }

        we_need_to_download = !config_file.data.installed_versions.contains_key(required_version);
    }

    let mut temp_version_folder: Option<tempfile::TempDir> = None;

    // Only download a new version if it isn't on the system already
    if we_need_to_download {
        temp_version_folder = Some(download_version_to_temp_folder(required_version, &version_db, paths)?);
    }

    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`add` command failed to load configuration data.")?;

    // If the version was added to the db while we downloaded it, we don't do anything. The temporary folder
    // for our download should be auto-deleted when the variable goes out of scope.
    if !config_file.data.installed_versions.contains_key(required_version) {
        // This is a very specific corner case: it means that between releasing the read lock and acquiring
        // the write lock the version was removed. In that case we re-download it while holding the write
        // lock. That is not ideal, but should be rare enough and guarantees success.
        if temp_version_folder.is_none() {
            temp_version_folder = Some(download_version_to_temp_folder(required_version, &version_db, paths)?);
        }
        
        let child_target_foldername = format!("julia-{}", required_version);
        let target_path = paths.juliauphome.join(&child_target_foldername);
        let temp_dir = temp_version_folder.unwrap();
        let source_path = temp_dir.path();
        std::fs::rename(&source_path, &target_path)
            .with_context(|| format!("Failed to rename temporary Julia download '{:?}' to '{:?}'", source_path, target_path))?;

        let mut rel_path = PathBuf::new();
        rel_path.push(".");
        rel_path.push(&child_target_foldername);

        config_file.data.installed_versions.insert(
            required_version.clone(),
            JuliaupConfigVersion {
                path: rel_path.to_string_lossy().into_owned(),
            },
        );
    }

    if !config_file.data.installed_channels.contains_key(channel) {
        config_file.data.installed_channels.insert(
            channel.to_string(),
            JuliaupConfigChannel::SystemChannel {
                version: required_version.clone(),
            },
        );    
    }
    else {
        eprintln!("'{}' is already installed.", &channel);
        return Ok(());
    }

    if config_file.data.default.is_none() {
        config_file.data.default = Some(channel.to_string());
    }

    #[cfg(not(windows))]
    let create_symlinks = config_file.data.settings.create_channel_symlinks;

    save_config_db(&mut config_file).with_context(|| {
        format!(
            "Failed to save configuration file from `add` command after '{}' was installed.",
            channel
        )
    })?;

    #[cfg(not(windows))]
    if create_symlinks {
        create_symlink(
            &JuliaupConfigChannel::SystemChannel {
                version: required_version.clone(),
            },
            &format!("julia-{}", channel),
            paths,
        )?;
    }

    Ok(())
}

fn add_nightly(channel: &str, paths: &GlobalPaths) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`add` command failed to load configuration data.")?;

    if config_file.data.installed_channels.contains_key(channel) {
        eprintln!("'{}' is already installed.", &channel);
        return Ok(());
    }

    let name = identify_nightly(&channel.to_string())?;
    let config_channel = install_nightly(channel, &name, paths)?;

    config_file
        .data
        .installed_channels
        .insert(channel.to_string(), config_channel.clone());

    if config_file.data.default.is_none() {
        config_file.data.default = Some(channel.to_string());
    }

    save_config_db(&mut config_file).with_context(|| {
        format!(
            "Failed to save configuration file from `add` command after '{channel}' was installed.",
        )
    })?;

    #[cfg(not(windows))]
    if config_file.data.settings.create_channel_symlinks {
        create_symlink(&config_channel, &format!("julia-{}", channel), paths)?;
    }
    Ok(())
}
