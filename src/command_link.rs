use crate::config_file::JuliaupConfigChannel;
use crate::config_file::{load_mut_config_db, save_config_db};
use crate::global_paths::GlobalPaths;
#[cfg(not(windows))]
use crate::operations::create_symlink;
use crate::versions_file::load_versions_db;
use anyhow::{bail, Context, Result};

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

    if versiondb_data.available_channels.contains_key(channel) {
        eprintln!("WARNING: The channel name `{}` is also a system channel. By linking your custom binary to this channel you are hiding this system channel.", channel);
    }

    config_file.data.installed_channels.insert(
        channel.to_string(),
        JuliaupConfigChannel::LinkedChannel {
            command: file.to_string(),
            args: Some(args.to_vec()),
        },
    );

    #[cfg(not(windows))]
    let create_symlinks = config_file.data.settings.create_channel_symlinks;

    save_config_db(&mut config_file)
        .with_context(|| "`link` command failed to save configuration db.")?;

    #[cfg(not(windows))]
    if create_symlinks {
        create_symlink(
            &JuliaupConfigChannel::LinkedChannel {
                command: file.to_string(),
                args: Some(args.to_vec()),
            },
            &format!("julia-{}", channel),
            paths,
        )?;
    }

    Ok(())
}
