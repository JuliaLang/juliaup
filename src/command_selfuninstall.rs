#[cfg(feature = "selfupdate")]
use anyhow::Result;

#[cfg(feature = "selfupdate")]
pub fn run_command_selfuninstall(paths: &crate::global_paths::GlobalPaths) -> Result<()> {
    use dialoguer::Confirm;

    use crate::{command_config_backgroundselfupdate::run_command_config_backgroundselfupdate, command_config_startupselfupdate::run_command_config_startupselfupdate, command_config_modifypath::run_command_config_modifypath, command_config_symlinks::run_command_config_symlinks};

    let choice = Confirm::new()
        .with_prompt("Do you really want to uninstall Julia?")
        .default(false)
        .interact()?;
    
    if !choice {
        return Ok(());
    }

    eprint!("Removing background self update task.");
    match run_command_config_backgroundselfupdate(Some(0), true, paths) {
        Ok(_) => eprintln!(" Success."),
        Err(_) => eprintln!(" Failed.")
    };

    eprint!("Removing startup self update configuration.");
    match run_command_config_startupselfupdate(Some(0), true, &paths) {
        Ok(_) => eprintln!(" Success."),
        Err(_) => eprintln!(" Failed.")
    };

    eprint!("Removing PATH modifications in startup scripts.");
    match run_command_config_modifypath(Some(false), true, &paths) {
        Ok(_) => eprintln!(" Success."),
        Err(_) => eprintln!(" Failed.")
    };

    eprint!("Removing symlinks.");
    match run_command_config_symlinks(Some(false), true, &paths) {
        Ok(_) => eprintln!(" Success."),
        Err(_) => eprintln!(" Failed.")
    };

    eprint!("Deleting Juliaup home folder {:?}.", paths.juliauphome);
    match std::fs::remove_dir_all(&paths.juliauphome) {
        Ok(_) => eprintln!(" Success."),
        Err(_) => eprintln!(" Failed.")
    };

    eprint!("Deleting Juliaup folder {:?}.", paths.juliaupselfhome);
    match std::fs::remove_dir_all(&paths.juliaupselfhome) {
        Ok(_) => eprintln!(" Success."),
        Err(_) => eprintln!(" Failed.")
    };

    eprintln!("Successfully removed Juliaup.");

    Ok(())
}
