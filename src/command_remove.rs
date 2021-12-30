use crate::{operations::{garbage_collect_versions, remove_symlink}, config_file::{load_mut_config_db, save_config_db}};
use anyhow::{bail, Context, Result};

pub fn run_command_remove(channel: String) -> Result<()> {
    let mut config_file = load_mut_config_db()
        .with_context(|| "`remove` command failed to load configuration data.")?;

    if !config_file.data.installed_channels.contains_key(&channel) {
        bail!("'{}' cannot be removed because it is currently not installed.", channel);
    }

    if let Some(ref default_value) = config_file.data.default {
        if &channel==default_value {
            bail!("'{}' cannot be removed because it is currently configured as the default channel.", channel);
        }
    }

    config_file.data.installed_channels.remove(&channel);

    if std::env::consts::OS != "windows" {
        remove_symlink(&format!("julia-{}", channel))?;
    }

    garbage_collect_versions(&mut config_file.data)?;

    save_config_db(&mut config_file)
        .with_context(|| format!("Failed to save configuration file from `remove` command after '{}' was installed.", channel))?;

    eprintln!("Julia '{}' successfully removed.", channel);

    Ok(())
}
