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
            "'{}' cannot be removed because it is currently not installed.",
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

    let x = config_file.data.installed_channels.get(channel).unwrap();

    if let JuliaupConfigChannel::DirectDownloadChannel { path, url, local_etag, server_etag, version } = x {
        let path_to_delete = paths.juliauphome.join(&path);

        let display = path_to_delete.display();

        if std::fs::remove_dir_all(&path_to_delete).is_err() {
            eprintln!("WARNING: Failed to delete {}. You can try to delete at a later point by running `juliaup gc`.", display)
        }
    };

    config_file.data.installed_channels.remove(channel);

    #[cfg(not(windows))]
    remove_symlink(&format!("julia-{}", channel))?;

    garbage_collect_versions(&mut config_file.data, paths)?;

    save_config_db(&mut config_file).with_context(|| {
        format!(
            "Failed to save configuration file from `remove` command after '{}' was installed.",
            channel
        )
    })?;

    eprintln!("Julia '{}' successfully removed.", channel);

    Ok(())
}
