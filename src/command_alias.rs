use crate::config_file::JuliaupConfigChannel;
use crate::config_file::{load_mut_config_db, save_config_db};
use crate::global_paths::GlobalPaths;
#[cfg(not(windows))]
use crate::operations::create_symlink;
use crate::operations::is_valid_channel;
use crate::versions_file::load_versions_db;
use anyhow::{bail, Context, Result};

pub fn run_command_alias(alias: &str, channel: &str, paths: &GlobalPaths) -> Result<()> {
    println!("alias: {}, channel: {}", alias, channel);
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`alias` command failed to load configuration data.")?;

    let versiondb_data =
        load_versions_db(paths).with_context(|| "`alias` command failed to load versions db.")?;

    if config_file.data.installed_channels.contains_key(alias) {
        bail!("Channel name `{}` is already used.", alias)
    }

    if !config_file.data.installed_channels.contains_key(channel) {
        eprintln!("WARNING: The channel `{}` does not currently exist. If this was a mistake, run `juliaup remove {}` and try again.", channel, alias);
    }

    if is_valid_channel(&versiondb_data, &alias.to_string())? {
        eprintln!("WARNING: The channel name `{}` is also a system channel. By creating an alias to this channel you are hiding this system channel.", alias);
    }

    config_file.data.installed_channels.insert(
        alias.to_string(),
        JuliaupConfigChannel::AliasedChannel {
            channel: channel.to_string(),
        },
    );

    #[cfg(not(windows))]
    let create_symlinks = config_file.data.settings.create_channel_symlinks;

    save_config_db(&mut config_file)
        .with_context(|| "`alias` command failed to save configuration db.")?;

    #[cfg(not(windows))]
    if create_symlinks {
        create_symlink(
            &JuliaupConfigChannel::AliasedChannel {
                channel: channel.to_string(),
            },
            &format!("julia-{}", channel),
            paths,
        )?;
    }

    Ok(())
}
