use anyhow::{bail, Context, Result};
use crate::config_file::{load_mut_config_db, save_config_db, load_config_db};

pub fn run_command_config_versionsdbupdate(value: Option<i64>, quiet: bool, paths: &crate::global_paths::GlobalPaths) -> Result<()> {
    match value {
        Some(value) => {
            if value < 0 {
                bail!("Invalid argument.");
            }

            let mut config_file = load_mut_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;
    
            let mut value_changed = false;

            if value != config_file.data.settings.versionsdb_update_interval {
                config_file.data.settings.versionsdb_update_interval = value;

                value_changed = true;
            }

            save_config_db(&mut config_file)
                .with_context(|| "Failed to save configuration file from `config` command.")?;

            if !quiet {
                if value_changed {
                    eprintln!("Property 'versionsdbupdateinterval' set to '{}'", value);
                }
                else {
                    eprintln!("Property 'versionsdbupdateinterval' is already set to '{}'", value);
                }
            }
        },
        None => {
            let config_file = load_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;

            if !quiet {
                eprintln!(
                    "Property 'versionsdbupdateinterval' set to '{}'", config_file.data.settings.versionsdb_update_interval);
            }
        }
    };

    Ok(())
}
