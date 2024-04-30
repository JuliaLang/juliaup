#[cfg(feature = "selfupdate")]
use anyhow::Result;

#[cfg(feature = "selfupdate")]
pub fn run_command_selfchannel(
    channel: crate::cli::JuliaupChannel,
    paths: &crate::global_paths::GlobalPaths,
) -> Result<()> {
    use crate::config_file::{load_mut_config_db, save_config_db};
    use anyhow::Context;

    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`self update` command failed to load configuration data.")?;

    config_file.self_data.juliaup_channel = Some(channel.to_lowercase().to_string());

    save_config_db(&mut config_file)
        .with_context(|| "`selfchannel` command failed to save configuration db.")?;

    Ok(())
}
