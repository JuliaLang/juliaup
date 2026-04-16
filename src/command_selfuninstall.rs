#[cfg(feature = "selfupdate")]
use anyhow::{Context, Result};

#[cfg(feature = "selfupdate")]
pub fn run_command_selfuninstall(paths: &crate::global_paths::GlobalPaths) -> Result<()> {
    use dialoguer::Confirm;

    use crate::{
        command_config_backgroundselfupdate::run_command_config_backgroundselfupdate,
        command_config_modifypath::run_command_config_modifypath,
        command_config_startupselfupdate::run_command_config_startupselfupdate,
        command_config_symlinks::run_command_config_symlinks,
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
    match run_command_config_backgroundselfupdate(Some(0), true, paths) {
        Ok(_) => eprintln!(" Success."),
        Err(e) => eprintln!(" Failed: {e}."),
    };

    eprint!("Removing startup self update configuration.");
    match run_command_config_startupselfupdate(Some(0), true, paths) {
        Ok(_) => eprintln!(" Success."),
        Err(e) => eprintln!(" Failed: {e}."),
    };

    eprint!("Removing PATH modifications in startup scripts.");
    match run_command_config_modifypath(Some(false), true, paths) {
        Ok(_) => eprintln!(" Success."),
        Err(e) => eprintln!(" Failed: {e}."),
    };

    eprint!("Removing symlinks.");
    match run_command_config_symlinks(Some(false), true, paths) {
        Ok(_) => eprintln!(" Success."),
        Err(e) => eprintln!(" Failed: {e}."),
    };

    eprint!(
        "Deleting Juliaup home folder {}.",
        paths.juliauphome.display()
    );
    match std::fs::remove_dir_all(&paths.juliauphome) {
        Ok(_) => eprintln!(" Success."),
        Err(e) => eprintln!(" Failed: {e}."),
    };

    if paths.juliauphome != paths.juliaupselfhome {
        let juliaup_binfolder_path = paths.juliaupselfhome.join("bin");
        let julia_symlink_path = juliaup_binfolder_path.join("julia");
        let julialauncher_path = juliaup_binfolder_path.join("julialauncher");
        let juliaup_path = juliaup_binfolder_path.join("juliaup");
        let juliaup_config_path = paths.juliaupselfhome.join("juliaupself.json");

        eprint!("Deleting julia symlink {}.", julia_symlink_path.display());
        match std::fs::remove_file(&julia_symlink_path) {
            Ok(_) => eprintln!(" Success."),
            Err(e) => eprintln!(" Failed: {e}."),
        };

        eprint!(
            "Deleting julialauncher binary {}.",
            julialauncher_path.display()
        );
        match std::fs::remove_file(&julialauncher_path) {
            Ok(_) => eprintln!(" Success."),
            Err(e) => eprintln!(" Failed: {e}."),
        };

        eprint!("Deleting juliaup binary {}.", juliaup_path.display());
        match std::fs::remove_file(&juliaup_path) {
            Ok(_) => eprintln!(" Success."),
            Err(e) => eprintln!(" Failed: {e}."),
        };

        if juliaup_binfolder_path
            .read_dir()
            .with_context(|| {
                format!(
                    "Failed to read Juliaup bin directory `{}`.",
                    juliaup_binfolder_path.display()
                )
            })?
            .next()
            .is_none()
        {
            eprint!(
                "Deleting the Juliaup bin folder {}.",
                juliaup_binfolder_path.display()
            );
            match std::fs::remove_dir(&juliaup_binfolder_path) {
                Ok(_) => {
                    eprintln!(" Success.");

                    eprint!(
                        "Deleting the Juliaup configuration file {}.",
                        juliaup_config_path.display()
                    );
                    match std::fs::remove_file(&juliaup_config_path) {
                        Ok(_) => eprintln!(" Success."),
                        Err(e) => eprintln!(" Failed: {e}."),
                    };

                    if paths
                        .juliaupselfhome
                        .read_dir()
                        .with_context(|| {
                            format!(
                                "Failed to read Juliaup folder `{}`.",
                                paths.juliaupselfhome.display()
                            )
                        })?
                        .next()
                        .is_none()
                    {
                        eprint!(
                            "Deleting the Juliaup folder {}.",
                            paths.juliaupselfhome.display()
                        );
                        match std::fs::remove_dir(&paths.juliaupselfhome) {
                            Ok(_) => eprintln!(" Success."),
                            Err(e) => {
                                eprintln!(
                                    " Failed: {e}, skipping removal of the entire Juliaup folder."
                                )
                            }
                        };
                    } else {
                        eprintln!(
                            "The Juliaup folder {} is not empty, skipping removal of the entire Juliaup folder.",
                            paths.juliaupselfhome.display()
                        );
                    }
                }
                Err(e) => {
                    eprintln!(" Failed: {e}, skipping removal of the entire Juliaup folder.")
                }
            };
        } else {
            eprintln!(
                "The Juliaup bin folder {} is not empty, skipping removal of the entire Juliaup folder.",
                juliaup_binfolder_path.display()
            );
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
use anyhow::Result;

#[cfg(not(feature = "selfupdate"))]
pub fn run_command_selfuninstall_unavailable() -> Result<()> {
    eprintln!(
        "Self uninstall command is unavailable in this variant of Juliaup.
This software was built with the intention of distributing it
through a package manager other than cargo or upstream."
    );
    Ok(())
}
