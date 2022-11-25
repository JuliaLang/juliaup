use crate::global_paths::GlobalPaths;
use crate::versions_file::load_versions_db;
use crate::config_file::{save_config_db, load_mut_config_db};
use anyhow::{bail,Context,Result};
use crate::config_file::JuliaupConfigChannel;
#[cfg(not(windows))]
use crate::operations::create_symlink;

pub fn run_command_link(channel: String, file: String, args: Vec<String>, paths: &GlobalPaths) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`link` command failed to load configuration data.")?;

    let versiondb_data = load_versions_db(paths)
        .with_context(|| "`link` command failed to load versions db.")?;

    if config_file.data.installed_channels.contains_key(&channel) {
        bail!("Channel name `{}` is already used.", channel)
    }

    if versiondb_data.available_channels.contains_key(&channel) {
        eprintln!("WARNING: The channel name `{}` is also a system channel. By linking your custom binary to this channel you are hiding this system channel.", channel);
    }

    config_file.data.installed_channels.insert(
        channel.clone(),
        JuliaupConfigChannel::LinkedChannel {
            command: file.clone(),
            args: Some(args.clone()),
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
                command: file.clone(),
                args: Some(args.clone()),
            },
            &format!("julia-{}", channel),
            paths,
        )?;
    }

    Ok(())
}
