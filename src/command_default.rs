use crate::config_file::*;
use anyhow::{anyhow, Context, Result};

pub fn run_command_default(channel: String) -> Result<()> {
    let mut config_data =
        load_config_db().with_context(|| "`default` command failed to load configuration db.")?;

    if !config_data.installed_channels.contains_key(&channel) {
        return Err(anyhow!("'{}' is not a valid Julia version.", channel));
    }

    config_data.default = Some(channel.clone());

    save_config_db(&config_data)
        .with_context(|| "`default` command failed to save configuration db.")?;

    eprintln!("Configured the default Julia version to be '{}'.", channel);

    Ok(())
}
