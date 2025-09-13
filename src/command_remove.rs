#[cfg(not(windows))]
use crate::operations::remove_symlink;
use crate::{
    config_file::{load_mut_config_db, save_config_db, JuliaupConfigChannel},
    global_paths::GlobalPaths,
    operations::garbage_collect_versions,
};
use anyhow::{bail, Context, Result};

pub fn run_command_remove(channel: &str, paths: &GlobalPaths) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`remove` command failed to load configuration data.")?;

    if !config_file.data.installed_channels.contains_key(channel) {
        bail!(
            "'{}' cannot be removed because it is not currently installed. Please run `juliaup list` to see available channels.",
            channel
        );
    }

    if let Some(ref default_value) = config_file.data.default {
        if channel == default_value {
            bail!(
                "'{}' cannot be removed because it is currently configured as the default channel.",
                channel
            );
        }
    }

    if config_file
        .data
        .overrides
        .iter()
        .any(|i| i.channel == channel)
    {
        bail!(
            "'{}' cannot be removed because it is currently used in a directory override.",
            channel
        );
    }

    let channel_info = config_file.data.installed_channels.get(channel).unwrap();

    // Determine what type of channel is being removed for better messaging
    let channel_type = match channel_info {
        JuliaupConfigChannel::DirectDownloadChannel { .. } => "channel".to_string(),
        JuliaupConfigChannel::SystemChannel { .. } => "channel".to_string(),
        JuliaupConfigChannel::LinkedChannel { .. } => "linked channel".to_string(),
        JuliaupConfigChannel::AliasChannel { target, .. } => {
            format!("alias (pointing to '{target}')")
        }
    };

    if let JuliaupConfigChannel::DirectDownloadChannel {
        path,
        url: _,
        local_etag: _,
        server_etag: _,
        version: _,
    } = channel_info
    {
        let path_to_delete = paths.juliauphome.join(path);

        let display = path_to_delete.display();

        if std::fs::remove_dir_all(&path_to_delete).is_err() {
            eprintln!("WARNING: Failed to delete {display}. You can try to delete at a later point by running `juliaup gc`.")
        }
    };

    config_file.data.installed_channels.remove(channel);

    #[cfg(not(windows))]
    remove_symlink(&format!("julia-{channel}"))?;

    garbage_collect_versions(false, &mut config_file.data, paths)?;

    save_config_db(&mut config_file).with_context(|| {
        format!(
            "Failed to save configuration file from `remove` command after '{channel}' was removed."
        )
    })?;

    eprintln!("Julia {channel_type} '{channel}' successfully removed.");

    Ok(())
}
