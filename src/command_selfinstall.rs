use anyhow::Result;

#[cfg(all(not(feature = "windowsstore"),feature = "selfupdate"))]
pub fn run_command_selfinstall() -> Result<()> {
    use itertools::Itertools;
    use std::{io::Write, process::Stdio};
    use anyhow::Context;

    let own_exe_path = std::env::current_exe()
        .with_context(|| "Could not determine the path of the running exe.")?;

    let my_own_path = own_exe_path.to_str().unwrap();
        
    match std::env::var("WSL_DISTRO_NAME") {
        // This is the WSL case, where we schedule a Windows task to do the update
        Ok(val) => {
            std::process::Command::new("schtasks.exe")
                .args([
                    "/create",
                    "/sc",
                    "hourly",
                    "/mo",
                    "5",
                    "/tn",
                    &format!("Juliaup self update for WSL {} distribution", val),
                    "/f",
                    "/it",
                    "/tr",
                    &format!("wsl --distribution {} {} self update", val, my_own_path)
                ])
                .output()
                .with_context(|| "Failed to create new Windows task for juliaup.")?;
        },
        Err(_e) => {
            let output = std::process::Command::new("crontab")
                .args(["-l"])
                .output()
                .with_context(|| "Failed to retrieve crontab configuration.")?;

            let new_crontab_content = String::from_utf8(output.stdout)?
                .lines()
                .filter(|x| !x.contains("4c79c12db1d34bbbab1f6c6f838f423f"))
                .chain([&format!("0 * * * * {} 4c79c12db1d34bbbab1f6c6f838f423f", my_own_path), ""])
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

    println!("Successfully created a background task that updates juliaup itself.");

    Ok(())
}

#[cfg(all(not(feature = "windowsstore"),not(feature = "selfupdate")))]
pub fn run_command_selfinstall() -> Result<()> {    
    println!("This command is not supported in this version of juliaup.");

    Ok(())
}

#[cfg(feature = "windowsstore")]
pub fn run_command_selfinstall() -> Result<()> {
    println!("This command is currently not supported in the Windows Store distributed version of juliaup.");

    Ok(())
}
