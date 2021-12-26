use crate::operations::install_version;
use crate::config_file::{JuliaupConfigChannel, load_mut_config_db, open_mut_config_file,save_config_db};
use crate::versions_file::load_versions_db;
use anyhow::{anyhow, bail, Context, Result};

pub fn run_command_add(channel: String) -> Result<()> {
    let version_db =
        load_versions_db().with_context(|| "`add` command failed to load versions db.")?;

    let required_version = &version_db
        .available_channels
        .get(&channel)
        .ok_or(anyhow!(
            "'{}' is not a valid Julia version or channel name.",
            &channel
        ))?
        .version;

    let file = open_mut_config_file()
        .with_context(|| "`add` command failed to open configuration file.")?;

    let (mut config_data, file_lock) = load_mut_config_db(&file)
        .with_context(|| "`add` command failed to load configuration data.")?;

    if config_data.installed_channels.contains_key(&channel) {
        bail!("'{}' is already installed.", &channel);
    }
    
    install_version(&required_version, &mut config_data, &version_db)?;

    config_data.installed_channels.insert(
        channel.clone(),
        JuliaupConfigChannel::SystemChannel {
            version: required_version.clone(),
        },
    );

    if config_data.default.is_none() {
        config_data.default = Some(channel.clone());
    }

    save_config_db(&file, config_data, file_lock)
        .with_context(|| format!("Failed to save configuration file from `add` command after '{}' was installed.", channel))?;

    Ok(())
}
