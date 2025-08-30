#[cfg(feature = "selfupdate")]
use anyhow::Result;

#[cfg(feature = "selfupdate")]
pub fn run_command_config_startupselfupdate(
    value: Option<i64>,
    quiet: bool,
    paths: &crate::global_paths::GlobalPaths,
) -> Result<()> {
    use crate::config_file::{load_config_db, load_mut_config_db, save_config_db};
    use anyhow::{bail, Context};

    match value {
        Some(value) => {
            if value < 0 {
                bail!("Invalid argument.");
            }

            let mut config_file = load_mut_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;

            let mut value_changed = false;

            let value = if value == 0 { None } else { Some(value) };

            if value != config_file.self_data.startup_selfupdate_interval {
                config_file.self_data.startup_selfupdate_interval = value;

                value_changed = true;
            }

            save_config_db(&mut config_file)
                .with_context(|| "Failed to save configuration file from `config` command.")?;

            if !quiet {
                if value_changed {
                    eprintln!(
                        "Property 'startupselfupdateinterval' set to '{}'",
                        value.unwrap_or(0)
                    );
                } else {
                    eprintln!(
                        "Property 'startupselfupdateinterval' is already set to '{}'",
                        value.unwrap_or(0)
                    );
                }
            }
        }
        None => {
            let config_file = load_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;

            if !quiet {
                eprintln!(
                    "Property 'startupselfupdateinterval' set to '{}'",
                    config_file.self_data.startup_selfupdate_interval.unwrap_or(0)
                );
            }
        }
    };

    Ok(())
}
