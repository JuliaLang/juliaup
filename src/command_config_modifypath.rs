#[cfg(feature = "selfupdate")]
pub fn run_command_config_modifypath(
    value: Option<bool>,
    quiet: bool,
    paths: &crate::global_paths::GlobalPaths,
) -> anyhow::Result<()> {
    use crate::config_file::{load_config_db, load_mut_config_db, save_config_db};
    use crate::operations::{
        add_binfolder_to_path_in_shell_scripts, remove_binfolder_from_path_in_shell_scripts,
    };
    use crate::utils::{print_juliaup_style, JuliaupMessageType};
    use anyhow::Context;

    match value {
        Some(value) => {
            let mut config_file = load_mut_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;

            let mut value_changed = false;

            if value != config_file.self_data.modify_path {
                config_file.self_data.modify_path = value;

                value_changed = true;
            }

            if value {
                add_binfolder_to_path_in_shell_scripts(&paths.juliaupselfbin)?;
            } else {
                remove_binfolder_from_path_in_shell_scripts()?;
            }

            save_config_db(&mut config_file)
                .with_context(|| "Failed to save configuration file from `config` command.")?;

            if !quiet {
                if value_changed {
                    print_juliaup_style(
                        "Configure",
                        &format!("Property 'modifypath' set to '{}'", value),
                        JuliaupMessageType::Success,
                    );
                } else {
                    print_juliaup_style(
                        "Configure",
                        &format!("Property 'modifypath' is already set to '{}'", value),
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
                        "Property 'modifypath' set to '{}'",
                        config_file.self_data.modify_path
                    ),
                    JuliaupMessageType::Success,
                );
            }
        }
    };

    Ok(())
}
