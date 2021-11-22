use crate::config_file::*;
use anyhow::{bail, Context, Result, anyhow};

pub fn run_command_selfchannel(channel: String) -> Result<()> {
    let mut config_data =
        load_config_db().with_context(|| "`selfupdate` command failed to load configuration db.")?;

    if channel != "dev" && channel != "releasepreview" && channel != "release" {
        bail!("'{}' is not a valid juliaup channel, you can only specify 'release', 'releasepreview' or 'dev'.", channel);
    }

    config_data.juliaup_channel = Some(channel.clone());

    save_config_db(&config_data)
        .with_context(|| "`selfchannel` command failed to save configuration db.")?;

    Ok(())
}
