use anyhow::Result;

#[cfg(all(not(feature = "windowsstore"),feature = "selfupdate"))]
pub fn run_command_selfuninstall() -> Result<()> {
    use crate::command_config_backgroundselfupdate::run_command_config_backgroundselfupdate;

    run_command_config_backgroundselfupdate(Some(0)).unwrap();

    println!("Successfully removed the background task that updates juliaup itself.");

    Ok(())
}

#[cfg(all(not(feature = "windowsstore"),not(feature = "selfupdate")))]
pub fn run_command_selfuninstall() -> Result<()> {    
    println!("This command is not supported in this version of juliaup.");

    Ok(())
}

#[cfg(feature = "windowsstore")]
pub fn run_command_selfuninstall() -> Result<()> {
    println!("This command is currently not supported in the Windows Store distributed version of juliaup.");

    Ok(())
}
