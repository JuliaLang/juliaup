#[cfg(feature = "selfupdate")]
use anyhow::Result;

#[cfg(feature = "selfupdate")]
pub fn run_command_selfuninstall(paths: &crate::global_paths::GlobalPaths) -> Result<()> {
    use requestty::{Question, prompt_one};

    use crate::{command_config_backgroundselfupdate::run_command_config_backgroundselfupdate, command_config_startupselfupdate::run_command_config_startupselfupdate, command_config_modifypath::run_command_config_modifypath, command_config_symlinks::run_command_config_symlinks};

    let question_confirm_uninstall = Question::confirm("uninstall")
        .message("Do you really want to uninstall Julia?")
        .default(false)
        .build();

    let answer = prompt_one(question_confirm_uninstall)?;
    
    if !answer.as_bool().unwrap() {
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
