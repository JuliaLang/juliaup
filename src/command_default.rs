use crate::operations::is_valid_channel;
use crate::versions_file::load_versions_db;
use crate::{config_file::*, global_paths::GlobalPaths};
use anyhow::{bail, Context, Result};

pub fn run_command_default(channel: &str, paths: &GlobalPaths) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`default` command failed to load configuration data.")?;

    if !config_file.data.installed_channels.contains_key(channel) {
        let version_db = load_versions_db(paths)
            .with_context(|| "`default` command failed to load versions db.")?;
        if !is_valid_channel(&version_db, &channel.to_string()) {
            bail!("'{}' is not a valid Julia version.", channel);
        } else {
            bail!(
                "'{}' is not an installed Julia version, run `juliaup add {}` first.",
                channel,
                channel
            );
        }
    }

    config_file.data.default = Some(channel.to_string());

    save_config_db(&mut config_file)
        .with_context(|| "`default` command failed to save configuration db.")?;

    eprintln!("Configured the default Julia version to be '{}'.", channel);

    Ok(())
}
