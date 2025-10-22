#[cfg(feature = "selfupdate")]
use crate::utils::{print_juliaup_style, JuliaupMessageType};
#[cfg(feature = "selfupdate")]
use anyhow::Result;

#[cfg(feature = "selfupdate")]
pub fn run_command_config_backgroundselfupdate(
    value: Option<i64>,
    quiet: bool,
    paths: &crate::global_paths::GlobalPaths,
) -> Result<()> {
    use crate::config_file::{load_config_db, load_mut_config_db, save_config_db};
    use crate::operations::{install_background_selfupdate, uninstall_background_selfupdate};
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

            if value != config_file.self_data.background_selfupdate_interval {
                config_file.self_data.background_selfupdate_interval = value;

                value_changed = true;

                match value {
                    Some(value) => {
                        install_background_selfupdate(value).unwrap();
                    }
                    None => {
                        uninstall_background_selfupdate().unwrap();
                    }
                }
            }

            save_config_db(&mut config_file)
                .with_context(|| "Failed to save configuration file from `config` command.")?;

            if !quiet {
                if value_changed {
                    print_juliaup_style(
                        "Configure",
                        &format!(
                            "Property 'backgroundselfupdateinterval' set to '{}'",
                            value.unwrap_or(0)
                        ),
                        JuliaupMessageType::Success,
                    );
                } else {
                    print_juliaup_style(
                        "Configure",
                        &format!(
                            "Property 'backgroundselfupdateinterval' is already set to '{}'",
                            value.unwrap_or(0)
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
                print_juliaup_style(
                    "Configure",
                    &format!(
                        "Property 'backgroundselfupdateinterval' set to '{}'",
                        config_file
                            .self_data
                            .background_selfupdate_interval
                            .unwrap_or(0)
                    ),
                    JuliaupMessageType::Success,
                );
            }
        }
    };

    Ok(())
}
