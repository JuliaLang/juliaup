#[cfg(not(target_os = "windows"))]
use anyhow::Result;

#[cfg(not(target_os = "windows"))]
pub fn run_command_config_startupselfupdate(value: Option<i64>) -> Result<()> {
    use crate::config_file::{load_mut_config_db, save_config_db, load_config_db};
    use anyhow::{bail, Context};

    match value {
        Some(value) => {
            if value < 0 {
                bail!("Invalid argument.");
            }

            let mut config_file = load_mut_config_db()
                .with_context(|| "`config` command failed to load configuration data.")?;
    
            let mut value_changed = false;

            let value = if value==0 {None} else {Some(value)};

            if value != config_file.data.settings.startup_selfupdate_interval {
                config_file.data.settings.startup_selfupdate_interval = value;

                value_changed = true;
            }

            save_config_db(&mut config_file)
                .with_context(|| "Failed to save configuration file from `config` command.")?;

            if value_changed {
                eprintln!("Property 'startupselfupdateinterval' set to '{}'", match value {
                    Some(value) => value,
                    None => 0
                });
            }
            else {
                eprintln!("Property 'startupselfupdateinterval' is already set to '{}'", match value {
                    Some(value) => value,
                    None => 0
                });
            }
        },
        None => {
            let config_data = load_config_db()
                .with_context(|| "`config` command failed to load configuration data.")?;

            eprintln!(
                "Property 'startupselfupdateinterval' set to '{}'",
                match config_data.settings.startup_selfupdate_interval {
                    Some(value) => value,
                    None => 0
                }
            );
        }
    };

    Ok(())
}
