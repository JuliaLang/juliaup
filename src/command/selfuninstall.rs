use anyhow::Result;

#[cfg(feature = "selfupdate")]
pub fn run(paths: &crate::global_paths::GlobalPaths) -> Result<()> {
    use dialoguer::Confirm;

    use crate::{
        command,
        utils::{print_juliaup_style, JuliaupMessageType},
    };

    let choice = Confirm::new()
        .with_prompt("Do you really want to uninstall Julia?")
        .default(false)
        .interact()?;

    if !choice {
        return Ok(());
    }

    eprint!("Removing background self update task.");
    match command::config::background_self_update(Some(0), true, paths) {
        Ok(_) => eprintln!(" Success."),
        Err(_) => eprintln!(" Failed."),
    };

    eprint!("Removing startup self update configuration.");
    match command::config::startup_self_update(Some(0), true, paths) {
        Ok(_) => eprintln!(" Success."),
        Err(_) => eprintln!(" Failed."),
    };

    eprint!("Removing PATH modifications in startup scripts.");
    match command::config::modify_path(Some(false), true, paths) {
        Ok(_) => eprintln!(" Success."),
        Err(_) => eprintln!(" Failed."),
    };

    eprint!("Removing symlinks.");
    match command::config::symlinks(Some(false), true, paths) {
        Ok(_) => eprintln!(" Success."),
        Err(_) => eprintln!(" Failed."),
    };

    eprint!("Deleting Juliaup home folder {:?}.", paths.juliauphome);
    match std::fs::remove_dir_all(&paths.juliauphome) {
        Ok(_) => eprintln!(" Success."),
        Err(_) => eprintln!(" Failed."),
    };

    if paths.juliauphome != paths.juliaupselfhome {
        let juliaup_binfolder_path = paths.juliaupselfhome.join("bin");
        let julia_symlink_path = juliaup_binfolder_path.join("julia");
        let julialauncher_path = juliaup_binfolder_path.join("julialauncher");
        let juliaup_path = juliaup_binfolder_path.join("juliaup");
        let juliaup_config_path = paths.juliaupselfhome.join("juliaupself.json");

        eprint!("Deleting julia symlink {:?}.", julia_symlink_path);
        match std::fs::remove_file(&julia_symlink_path) {
            Ok(_) => eprintln!(" Success."),
            Err(_) => eprintln!(" Failed."),
        };

        eprint!("Deleting julialauncher binary {:?}.", julialauncher_path);
        match std::fs::remove_file(&julialauncher_path) {
            Ok(_) => eprintln!(" Success."),
            Err(_) => eprintln!(" Failed."),
        };

        eprint!("Deleting juliaup binary {:?}.", juliaup_path);
        match std::fs::remove_file(&juliaup_path) {
            Ok(_) => eprintln!(" Success."),
            Err(_) => eprintln!(" Failed."),
        };

        if juliaup_binfolder_path.read_dir()?.next().is_none() {
            eprint!(
                "Deleting the Juliaup bin folder {:?}.",
                juliaup_binfolder_path
            );
            match std::fs::remove_dir(&juliaup_binfolder_path) {
                Ok(_) => {
                    eprintln!(" Success.");

                    eprint!(
                        "Deleting the Juliaup configuration file {:?}.",
                        juliaup_config_path
                    );
                    match std::fs::remove_file(&juliaup_config_path) {
                        Ok(_) => eprintln!(" Success."),
                        Err(_) => eprintln!(" Failed."),
                    };

                    if paths.juliaupselfhome.read_dir()?.next().is_none() {
                        eprint!("Deleting the Juliaup folder {:?}.", paths.juliaupselfhome);
                        match std::fs::remove_dir(&paths.juliaupselfhome) {
                            Ok(_) => eprintln!(" Success."),
                            Err(_) => {
                                eprintln!(" Failed, skipping removal of the entire Juliaup folder.")
                            }
                        };
                    } else {
                        eprintln!("The Juliaup folder {:?} is not empty, skipping removal of the entire Juliaup folder.", paths.juliaupselfhome);
                    }
                }
                Err(_) => eprintln!(" Failed, skipping removal of the entire Juliaup folder."),
            };
        } else {
            eprintln!("The Juliaup bin folder {:?} is not empty, skipping removal of the entire Juliaup folder.", juliaup_binfolder_path);
        }
    }

    print_juliaup_style(
        "Remove",
        "Juliaup successfully removed.",
        JuliaupMessageType::Success,
    );

    Ok(())
}

#[cfg(not(feature = "selfupdate"))]
pub fn unavailable() -> Result<()> {
    eprintln!(
        "Self uninstall command is unavailable in this variant of Juliaup.
This software was built with the intention of distributing it
through a package manager other than cargo or upstream."
    );
    Ok(())
}
