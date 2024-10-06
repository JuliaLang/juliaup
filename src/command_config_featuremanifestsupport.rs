use crate::config_file::{load_config_db, load_mut_config_db, save_config_db};
use anyhow::{Context, Result};

pub fn run_command_config_featuremanifestsupport(
    value: Option<bool>,
    quiet: bool,
    paths: &crate::global_paths::GlobalPaths,
) -> Result<()> {
    match value {
        Some(value) => {
            let mut config_file = load_mut_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;

            let mut value_changed = false;

            if value != config_file.data.settings.feature_manifest_support {
                config_file.data.settings.feature_manifest_support = value;

                value_changed = true;
            }

            save_config_db(&mut config_file)
                .with_context(|| "Failed to save configuration file from `config` command.")?;

            if !quiet {
                if value_changed {
                    eprintln!("Property 'featuremanifestsupport' set to '{}'", value);
                } else {
                    eprintln!(
                        "Property 'featuremanifestsupport' is already set to '{}'",
                        value
                    );
                }
            }
        }
        None => {
            let config_file = load_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;

            if !quiet {
                eprintln!(
                    "Property 'featuremanifestsupport' set to '{}'",
                    config_file.data.settings.feature_manifest_support
                );
            }
        }
    };

    Ok(())
}
