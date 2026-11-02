use crate::config_file::JuliaupConfigChannel;
use crate::config_file::{load_mut_config_db, save_config_db};
use crate::global_paths::GlobalPaths;
use crate::operations::is_valid_channel;
#[cfg(not(windows))]
use crate::operations::{create_symlink, remove_symlink};
use crate::utils::is_valid_julia_path;
use crate::utils::{print_juliaup_style, JuliaupMessageType};
use crate::versions_file::load_versions_db;
use anyhow::{bail, Context, Result};
use path_absolutize::Absolutize;
use std::io::IsTerminal;
use std::path::Path;

/// Decide whether the existing channel `channel` may be replaced by the new
/// link. Returns `Ok(false)` if the user declined at the prompt.
///
/// Only linked channels and aliases can be replaced. Installed Julia versions
/// have files on disk that `juliaup remove` cleans up, so they are left alone.
fn confirm_replace(existing: &JuliaupConfigChannel, channel: &str, force: bool) -> Result<bool> {
    match existing {
        JuliaupConfigChannel::LinkedChannel { .. } | JuliaupConfigChannel::AliasChannel { .. } => {}
        JuliaupConfigChannel::SystemChannel { .. }
        | JuliaupConfigChannel::DirectDownloadChannel { .. } => bail!(
            "Channel name `{}` is already used by an installed Julia version. Run `juliaup remove {}` first if you want to replace it with a link.",
            channel,
            channel
        ),
    }

    if force {
        return Ok(true);
    }

    if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        bail!(
            "Channel name `{}` is already used. Run `juliaup link --force {} ...` to replace it.",
            channel,
            channel
        );
    }

    let replace = dialoguer::Confirm::new()
        .with_prompt(format!(
            "Channel `{}` already exists, do you want to replace the existing channel?",
            channel
        ))
        .default(false)
        .interact()?;

    Ok(replace)
}

pub fn run_command_link(
    channel: &str,
    target: &str,
    args: &[String],
    force: bool,
    paths: &GlobalPaths,
) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`link` command failed to load configuration data.")?;

    let versiondb_data =
        load_versions_db(paths).with_context(|| "`link` command failed to load versions db.")?;

    // Validate an alias target before asking anything, so that the user is
    // never asked to confirm a replacement that would fail anyway.
    if let Some(target_channel) = target.strip_prefix('+') {
        if target_channel == channel {
            bail!("Cannot make channel `{}` an alias of itself.", channel);
        }

        // Validate that the target channel exists and is not an alias
        if let Some(target_info) = config_file.data.installed_channels.get(target_channel) {
            // Prevent alias-to-alias chains for simplicity and maintainability
            if let JuliaupConfigChannel::AliasChannel { .. } = target_info {
                bail!("Cannot create an alias to another alias `{}`. Please create an alias directly to the target channel instead.", target_channel);
            }
        } else if !is_valid_channel(&versiondb_data, target_channel) {
            bail!("Target channel `{}` is not installed and is not a valid system channel. Please run `juliaup add {}` first or check `juliaup list` for available channels.", target_channel, target_channel);
        }

        // For the same reason, a channel that other aliases point to cannot
        // itself become an alias.
        let pointing_aliases: Vec<&String> = config_file
            .data
            .installed_channels
            .iter()
            .filter_map(|(name, info)| match info {
                JuliaupConfigChannel::AliasChannel { target: t, .. } if t == channel => Some(name),
                _ => None,
            })
            .collect();
        if !pointing_aliases.is_empty() {
            bail!(
                "Cannot turn `{}` into an alias because other aliases point to it: {:?}.",
                channel,
                pointing_aliases
            );
        }
    }

    // Replacing an existing link keeps its name, so a default or directory
    // override that refers to it keeps working
    // (https://github.com/JuliaLang/juliaup/issues/584).
    let replacing = match config_file.data.installed_channels.get(channel) {
        None => false,
        Some(existing) => {
            if !confirm_replace(existing, channel, force)? {
                eprintln!("Channel `{}` was not changed.", channel);
                return Ok(());
            }
            true
        }
    };

    if replacing {
        print_juliaup_style(
            "Replacing",
            &format!("existing channel `{}`.", channel),
            JuliaupMessageType::Progress,
        );
    }

    if is_valid_channel(&versiondb_data, channel) {
        eprintln!("WARNING: The channel name `{channel}` is also a system channel. By linking your custom binary to this channel you are hiding this system channel.");
    }

    // Check if this is a channel alias (starts with +)
    if let Some(target_channel) = target.strip_prefix('+') {
        config_file.data.installed_channels.insert(
            channel.to_string(),
            JuliaupConfigChannel::AliasChannel {
                target: target_channel.to_string(),
                args: if args.is_empty() {
                    None
                } else {
                    Some(args.to_vec())
                },
            },
        );

        if args.is_empty() {
            print_juliaup_style(
                "Link",
                &format!("Channel alias `{channel}` created, pointing to `{target_channel}`."),
                JuliaupMessageType::Success,
            );
        } else {
            print_juliaup_style("Link", &format!("Channel alias `{channel}` created, pointing to `{target_channel}` with args: {:?}.", args), JuliaupMessageType::Success);
        }
    } else {
        let absolute_file_path = Path::new(target)
            .absolutize()
            .with_context(|| format!("Failed to convert path `{target}` to absolute path."))?;

        if !is_valid_julia_path(&absolute_file_path.to_path_buf()) {
            eprintln!("WARNING: There is no julia binary at {}. If this was a mistake, run `juliaup link --force {} <path>` with the correct path.", absolute_file_path.to_string_lossy(), channel);
        }

        config_file.data.installed_channels.insert(
            channel.to_string(),
            JuliaupConfigChannel::LinkedChannel {
                command: absolute_file_path.to_string_lossy().to_string(),
                args: Some(args.to_vec()),
            },
        );

        print_juliaup_style(
            "Link",
            &format!(
                "Channel `{}` linked to `{}`.",
                channel,
                absolute_file_path.to_string_lossy()
            ),
            JuliaupMessageType::Success,
        );
    }

    #[cfg(not(windows))]
    let create_symlinks = config_file.data.settings.create_channel_symlinks;

    save_config_db(&mut config_file, paths).with_context(|| {
        format!(
            "`link` command failed to save configuration db at `{}`.",
            paths.juliaupconfig.display()
        )
    })?;

    // An alias has no `julia-<channel>` shim of its own, so drop the one that
    // belonged to the linked channel it replaces.
    #[cfg(not(windows))]
    if create_symlinks && replacing && target.starts_with('+') {
        remove_symlink(&format!("julia-{channel}"))?;
    }

    #[cfg(not(windows))]
    if create_symlinks && !target.starts_with('+') {
        let absolute_file_path = Path::new(target)
            .absolutize()
            .with_context(|| format!("Failed to convert path `{target}` to absolute path."))?;

        create_symlink(
            &JuliaupConfigChannel::LinkedChannel {
                command: absolute_file_path.to_string_lossy().to_string(),
                args: Some(args.to_vec()),
            },
            &format!("julia-{channel}"),
            paths,
        )?;
    }

    Ok(())
}
