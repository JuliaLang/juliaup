use crate::config_file::{load_config_db, load_mut_config_db, save_config_db};
use crate::utils::{print_juliaup_style, JuliaupMessageType};
use anyhow::{bail, Context, Result};

pub fn run_command_config_versionsdbupdate(
    value: Option<i64>,
    quiet: bool,
    paths: &crate::global_paths::GlobalPaths,
) -> Result<()> {
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
                    print_juliaup_style(
                        "Configure",
                        &format!("Property 'versionsdbupdateinterval' set to '{}'", value),
                        JuliaupMessageType::Success,
                    );
                } else {
                    print_juliaup_style(
                        "Configure",
                        &format!(
                            "Property 'versionsdbupdateinterval' is already set to '{}'",
                            value
                        ),
                        JuliaupMessageType::Success,
                    );
                }
            }
        }
        None => {
            let config_file = load_config_db(paths, None)
                .with_context(|| "`config` command failed to load configuration data.")?;

            if !quiet {
                eprintln!(
                    "Property 'versionsdbupdateinterval' set to '{}'",
                    config_file.data.settings.versionsdb_update_interval
                );
            }
        }
    };

    Ok(())
}
