use crate::config_file::JuliaupConfigChannel;
use crate::config_file::{load_mut_config_db, save_config_db};
use crate::global_paths::GlobalPaths;
#[cfg(not(windows))]
use crate::operations::create_symlink;
use crate::operations::is_valid_channel;
use crate::utils::is_valid_julia_path;
use crate::versions_file::load_versions_db;
use anyhow::{bail, Context, Result};
use path_absolutize::Absolutize;
use std::path::Path;

pub fn run_command_link(
    channel: &str,
    file: &str,
    args: &[String],
    paths: &GlobalPaths,
) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`link` command failed to load configuration data.")?;

    let versiondb_data =
        load_versions_db(paths).with_context(|| "`link` command failed to load versions db.")?;

    if config_file.data.installed_channels.contains_key(channel) {
        bail!("Channel name `{}` is already used.", channel)
    }

    if is_valid_channel(&versiondb_data, &channel.to_string())? {
        eprintln!("WARNING: The channel name `{channel}` is also a system channel. By linking your custom binary to this channel you are hiding this system channel.");
    }

    // Check if this is a channel alias (starts with +)
    if let Some(target_channel) = file.strip_prefix('+') {
        // Remove the + prefix

        // Validate that the target channel exists or is valid
        if !config_file
            .data
            .installed_channels
            .contains_key(target_channel)
            && !is_valid_channel(&versiondb_data, &target_channel.to_string())?
        {
            bail!("Target channel `{}` is not installed and is not a valid system channel. Please run `juliaup add {}` first or check `juliaup list` for available channels.", target_channel, target_channel);
        }

        if !args.is_empty() {
            bail!("Arguments are not supported when creating channel aliases. Remove the extra arguments: {:?}", args);
        }

        config_file.data.installed_channels.insert(
            channel.to_string(),
            JuliaupConfigChannel::AliasChannel {
                target: target_channel.to_string(),
            },
        );

        eprintln!("Channel alias `{channel}` created, pointing to `{target_channel}`.");
    } else {
        // Original behavior for linking to binary files
        let absolute_file_path = Path::new(file)
            .absolutize()
            .with_context(|| format!("Failed to convert path `{file}` to absolute path."))?;

        if !is_valid_julia_path(&absolute_file_path.to_path_buf()) {
            eprintln!("WARNING: There is no julia binary at {}. If this was a mistake, run `juliaup remove {}` and try again.", absolute_file_path.to_string_lossy(), channel);
        }

        config_file.data.installed_channels.insert(
            channel.to_string(),
            JuliaupConfigChannel::LinkedChannel {
                command: absolute_file_path.to_string_lossy().to_string(),
                args: Some(args.to_vec()),
            },
        );

        eprintln!(
            "Channel `{}` linked to `{}`.",
            channel,
            absolute_file_path.to_string_lossy()
        );
    }

    #[cfg(not(windows))]
    let create_symlinks = config_file.data.settings.create_channel_symlinks;

    save_config_db(&mut config_file)
        .with_context(|| "`link` command failed to save configuration db.")?;

    #[cfg(not(windows))]
    if create_symlinks && !file.starts_with('+') {
        // Only create symlinks for binary links, not channel aliases
        create_symlink(
            &JuliaupConfigChannel::LinkedChannel {
                command: file.to_string(),
                args: Some(args.to_vec()),
            },
            &format!("julia-{channel}"),
            paths,
        )?;
    }

    Ok(())
}
