use anyhow::Result;

pub fn run_command_config_checkchanneluptodate(
    value: Option<bool>,
    quiet: bool,
    paths: &crate::global_paths::GlobalPaths,
) -> Result<()> {
    use crate::config_file::{load_config_db, load_mut_config_db, save_config_db};
    use anyhow::Context;

    match value {
        Some(value) => {
            let mut config_file = load_mut_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;

            let mut value_changed = false;

            if value != config_file.data.settings.should_check_channel_uptodate {
                config_file.data.settings.should_check_channel_uptodate = value;
                value_changed = true;
            }

            save_config_db(&mut config_file)
                .with_context(|| "Failed to save configuration file from `config` command.")?;

            if !quiet {
                if value_changed {
                    eprintln!("Property 'checkchanneluptodate' set to '{}'", value);
                } else {
                    eprintln!(
                        "Property 'checkchanneluptodate' is already set to '{}'",
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
                    "Property 'checkchanneluptodate' set to '{}'",
                    config_file.data.settings.should_check_channel_uptodate
                );
            }
        }
    };

    Ok(())
}
