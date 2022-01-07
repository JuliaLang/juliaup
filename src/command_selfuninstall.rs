#[cfg(feature = "selfupdate")]
use anyhow::Result;

#[cfg(feature = "selfupdate")]
pub fn run_command_selfuninstall(paths: &crate::global_paths::GlobalPaths) -> Result<()> {
    use crate::command_config_backgroundselfupdate::run_command_config_backgroundselfupdate;

    run_command_config_backgroundselfupdate(Some(0), false, paths).unwrap();

    println!("Successfully removed the background task that updates juliaup itself.");

    Ok(())
}
