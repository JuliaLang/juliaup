use anyhow::Result;

pub fn run_command_config_manifestversiondetect(
    value: Option<bool>,
    quiet: bool,
    paths: &crate::global_paths::GlobalPaths,
) -> Result<()> {
    use crate::config_file::{load_config_db, load_mut_config_db, save_config_db};
    use crate::utils::{print_juliaup_style, JuliaupMessageType};
    use anyhow::Context;

    match value {
        Some(value) => {
            let mut config_file = load_mut_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;

            let mut value_changed = false;

            if value != config_file.data.settings.manifest_version_detect {
                config_file.data.settings.manifest_version_detect = value;
                value_changed = true;
            }

            save_config_db(&mut config_file)
                .with_context(|| "Failed to save configuration file from `config` command.")?;

            if !quiet {
                if value_changed {
                    print_juliaup_style(
                        "Configure",
                        &format!("Property 'manifestversiondetect' set to '{}'", value),
                        JuliaupMessageType::Success,
                    );
                } else {
                    print_juliaup_style(
                        "Configure",
                        &format!(
                            "Property 'manifestversiondetect' is already set to '{}'",
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
                print_juliaup_style(
                    "Configure",
                    &format!(
                        "Property 'manifestversiondetect' set to '{}'",
                        config_file.data.settings.manifest_version_detect
                    ),
                    JuliaupMessageType::Success,
                );
            }
        }
    };

    Ok(())
}
