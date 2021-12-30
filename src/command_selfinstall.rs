#[cfg(feature = "selfupdate")]
use anyhow::Result;

#[cfg(feature = "selfupdate")]
pub fn run_command_selfinstall() -> Result<()> {
    use crate::command_config_backgroundselfupdate::run_command_config_backgroundselfupdate;

    run_command_config_backgroundselfupdate(Some(60)).unwrap();

    println!("Successfully created a background task that updates juliaup itself.");

    Ok(())
}
