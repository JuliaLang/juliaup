#[cfg(feature = "selfupdate")]
use anyhow::Result;

#[cfg(feature = "selfupdate")]
pub fn run_command_selfchannel(
    channel: String,
    paths: &crate::global_paths::GlobalPaths,
) -> Result<()> {
    use crate::config_file::{load_mut_config_db, save_config_db};
    use anyhow::{bail, Context};

    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`self update` command failed to load configuration data.")?;

    if channel != "dev" && channel != "releasepreview" && channel != "release" {
        bail!("'{}' is not a valid juliaup channel, you can only specify 'release', 'releasepreview' or 'dev'.", channel);
    }

    config_file.self_data.juliaup_channel = Some(channel.clone());

    save_config_db(&mut config_file)
        .with_context(|| "`selfchannel` command failed to save configuration db.")?;

    Ok(())
}
