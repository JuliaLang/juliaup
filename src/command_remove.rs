use crate::operations::garbage_collect_versions;
use crate::config_file::*;
use anyhow::{bail, Context, Result};

pub fn run_command_remove(channel: String) -> Result<()> {
    let file = open_mut_config_file()
        .with_context(|| "`remove` command failed to open configuration file.")?;
    
    let (mut config_data, file_lock) = load_mut_config_db(&file)
        .with_context(|| "`remove` command failed to load configuration data.")?;

    if !config_data.installed_channels.contains_key(&channel) {
        bail!("'{}' cannot be removed because it is currently not installed.", channel);
    }

    if let Some(ref default_value) = config_data.default {
        if &channel==default_value {
            bail!("'{}' cannot be removed because it is currently configured as the default channel.", channel);
        }
    }

    config_data.installed_channels.remove(&channel);

    garbage_collect_versions(&mut config_data)?;

    save_config_db(&file, config_data, file_lock)
        .with_context(|| format!("Failed to save configuration file from `remove` command after '{}' was installed.", channel))?;

    eprintln!("Julia '{}' successfully removed.", channel);

    Ok(())
}
