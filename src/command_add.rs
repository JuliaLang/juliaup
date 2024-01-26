use crate::config_file::{load_mut_config_db, save_config_db, JuliaupConfigChannel};
use crate::global_paths::GlobalPaths;
#[cfg(not(windows))]
use crate::operations::create_symlink;
use crate::operations::{identify_nightly, install_nightly, install_version, update_version_db};
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

    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`add` command failed to load configuration data.")?;

    if config_file.data.installed_channels.contains_key(channel) {
        eprintln!("'{}' is already installed.", &channel);
        return Ok(());
    }

    install_version(required_version, &mut config_file.data, &version_db, paths)?;

    config_file.data.installed_channels.insert(
        channel.to_string(),
        JuliaupConfigChannel::SystemChannel {
            version: required_version.clone(),
        },
    );

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
    let config_channel = install_nightly(channel, &name, &mut config_file.data, paths)?;

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
