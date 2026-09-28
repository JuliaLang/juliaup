#[cfg(feature = "selfupdate")]
pub fn run_command_config_applinks(
    value: Option<bool>,
    quiet: bool,
    paths: &crate::global_paths::GlobalPaths,
) -> anyhow::Result<()> {
    use crate::app_links::{create_app_links, depot_from_env, remove_app_links};
    use crate::config_file::{load_config_db, load_mut_config_db, save_config_db};
    use crate::utils::{print_juliaup_style, JuliaupMessageType};
    use anyhow::Context;

    match value {
        Some(value) => {
            let mut config_file = load_mut_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;

            let mut value_changed = false;

            if value != config_file.self_data.app_links {
                config_file.self_data.app_links = value;

                value_changed = true;
            }

            if value {
                let depot = depot_from_env();
                create_app_links(&paths.juliaupselfbin, depot.as_deref())?;
                config_file.self_data.app_links_depot = depot;
            } else {
                remove_app_links()?;
                config_file.self_data.app_links_depot = None;
            }

            save_config_db(&mut config_file, paths).with_context(|| {
                format!(
                    "Failed to save configuration file from `config` command at `{}`.",
                    paths.juliaupconfig.display()
                )
            })?;

            if !quiet {
                if value_changed {
                    print_juliaup_style(
                        "Configure",
                        &format!("Property 'applinks' set to '{}'", value),
                        JuliaupMessageType::Success,
                    );
                } else {
                    print_juliaup_style(
                        "Configure",
                        &format!("Property 'applinks' is already set to '{}'", value),
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
                        "Property 'applinks' set to '{}'",
                        config_file.self_data.app_links
                    ),
                    JuliaupMessageType::Success,
                );
            }
        }
    };

    Ok(())
}
