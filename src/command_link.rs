use crate::versions_file::load_versions_db;
use crate::config_file::{load_config_db,save_config_db};
use anyhow::{bail,Context,Result};
use crate::config_file::JuliaupConfigChannel;

pub fn run_command_link(channel: String, file: String, args: Vec<String>) -> Result<()> {
    let mut config_data = load_config_db()
        .with_context(|| "`status` command failed to load configuration file.")?;

    let versiondb_data = load_versions_db()
        .with_context(|| "`status` command failed to load versions db.")?;

    if config_data.installed_channels.contains_key(&channel) {
        bail!("Channel name `{}` is already used.", channel)
    }

    if versiondb_data.available_channels.contains_key(&channel) {
        eprintln!("WARNING: The channel name `{}` is also a system channel. By linking your custom binary to this channel you are hiding this system channel.", channel);
    }

    config_data.installed_channels.insert(channel, JuliaupConfigChannel::LinkedChannel {command: file.clone(), args: Some(args.clone())});

    save_config_db(&config_data)
        .with_context(|| "`link` command failed to save configuration db.")?;

    Ok(())
}
