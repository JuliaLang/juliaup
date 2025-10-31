use anyhow::{anyhow, Result};

pub fn run_command_config_autoinstall(
    value: Option<String>,
    quiet: bool,
    paths: &crate::global_paths::GlobalPaths,
) -> Result<()> {
    use crate::config_file::{load_config_db, load_mut_config_db, save_config_db};
    use crate::utils::{print_juliaup_style, JuliaupMessageType};
    use anyhow::Context;

    match value {
        Some(value_str) => {
            let mut config_file = load_mut_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;

            let mut value_changed = false;
            let new_value = match value_str.to_lowercase().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                "default" => None,
                _ => {
                    return Err(anyhow!(
                        "Invalid value '{}'. Valid values are: true, false, default (to unset the property)",
                        value_str
                    ))
                }
            };

            if new_value != config_file.data.settings.auto_install_channels {
                config_file.data.settings.auto_install_channels = new_value;
                value_changed = true;
            }

            save_config_db(&mut config_file)
                .with_context(|| "Failed to save configuration file from `config` command.")?;

            if !quiet {
                let display_value = new_value
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| "default (not set)".to_string());

                if value_changed {
                    print_juliaup_style(
                        "Configure",
                        &format!("Property 'autoinstallchannels' set to '{}'", display_value),
                        JuliaupMessageType::Success,
                    );
                } else {
                    print_juliaup_style(
                        "Configure",
                        &format!(
                            "Property 'autoinstallchannels' is already set to '{}'",
                            display_value
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
                        "Property 'autoinstallchannels' set to '{}'",
                        config_file
                            .data
                            .settings
                            .auto_install_channels
                            .map(|b| b.to_string())
                            .unwrap_or_else(|| "default (not set)".to_string())
                    ),
                    JuliaupMessageType::Success,
                );
            }
        }
    };

    Ok(())
}
