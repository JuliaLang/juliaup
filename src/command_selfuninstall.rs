#[cfg(feature = "selfupdate")]
use anyhow::Result;

#[cfg(feature = "selfupdate")]
pub fn run_command_selfuninstall() -> Result<()> {
    use crate::command_config_backgroundselfupdate::run_command_config_backgroundselfupdate;

    run_command_config_backgroundselfupdate(Some(0), false).unwrap();

    println!("Successfully removed the background task that updates juliaup itself.");

    Ok(())
}
