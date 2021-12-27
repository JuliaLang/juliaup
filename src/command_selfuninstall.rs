use anyhow::{Context, Result};
use std::{io::Write, process::Stdio};
use itertools::Itertools;


#[cfg(feature = "selfupdate")]
#[cfg(not(feature = "windowsstore"))]
pub fn run_command_selfuninstall() -> Result<()> {
    match std::env::var("WSL_DISTRO_NAME") {
        // This is the WSL case, where we schedule a Windows task to do the update
        Ok(val) => {            
            std::process::Command::new("schtasks.exe")
                .args([
                    "/delete",
                    "/tn",
                    &format!("Juliaup self update for WSL {} distribution", val),
                    "/f",
                ])
                .output()
                .with_context(|| "Failed to remove Windows task for juliaup.")?;

        },
        Err(_e) => {
            let output = std::process::Command::new("crontab")
                .args(["-l"])
                .output()
                .with_context(|| "Failed to remove cron task.")?;

            let new_crontab_content = String::from_utf8(output.stdout)?
                .lines()
                .filter(|x| !x.contains("4c79c12db1d34bbbab1f6c6f838f423f"))
                .chain([""])
                .join("\n");

            let mut child = std::process::Command::new("crontab")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;

            let child_stdin = child.stdin.as_mut().unwrap();

            child_stdin.write_all(new_crontab_content.as_bytes())?;

            // Close stdin to finish and avoid indefinite blocking
            drop(child_stdin);
                
            child.wait_with_output()?;
        },
    };

    println!("Successfully removed the background task that updates juliaup itself.");

    Ok(())
}

#[cfg(not(feature = "selfupdate"))]
pub fn run_command_selfuninstall() -> Result<()> {    
    println!("This command is not supported in this version of juliaup.");

    Ok(())
}

#[cfg(feature = "windowsstore")]
pub fn run_command_selfuninstall() -> Result<()> {
    println!("This command is currently not supported in the Windows Store distributed version of juliaup.");

    Ok(())
}
