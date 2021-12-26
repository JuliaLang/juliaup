use crate::config_file::*;
use crate::versions_file::load_versions_db;
use anyhow::{bail, Context, Result};

pub fn run_command_default(channel: String) -> Result<()> {
    let mut config_file = load_mut_config_db()
        .with_context(|| "`default` command failed to load configuration data.")?;

    if !config_file.data.installed_channels.contains_key(&channel) {
        let version_db = load_versions_db().with_context(|| "`default` command failed to load versions db.")?;
        if !version_db.available_channels.contains_key(&channel) {
            bail!("'{}' is not a valid Julia version.", channel);
        } else {
            bail!("'{}' is not an installed Julia version, run `juliaup add {}` first.", channel, channel);
        }
    }

    config_file.data.default = Some(channel.clone());

    save_config_db(config_file)
        .with_context(|| "`default` command failed to save configuration db.")?;

    eprintln!("Configured the default Julia version to be '{}'.", channel);

    Ok(())
}
