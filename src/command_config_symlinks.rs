#[cfg(not(windows))]
use anyhow::Result;

#[cfg(not(windows))]
pub fn run_command_config_symlinks(
    value: Option<bool>,
    quiet: bool,
    paths: &crate::global_paths::GlobalPaths,
) -> Result<()> {
    use crate::config_file::{load_config_db, load_mut_config_db, save_config_db};
    use crate::operations::{create_symlink, remove_symlink};
    use anyhow::Context;

    match value {
        Some(value) => {
            let mut config_file = load_mut_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;

            let mut value_changed = false;

            if value != config_file.data.settings.create_channel_symlinks {
                config_file.data.settings.create_channel_symlinks = value;
                value_changed = true;

                for (channel_name, channel) in &config_file.data.installed_channels {
                    if value {
                        create_symlink(channel, &format!("julia-{}", channel_name), paths)?;
                    } else {
                        remove_symlink(&format!("julia-{}", channel_name))?;
                    }
                }
            }

            save_config_db(&mut config_file)
                .with_context(|| "Failed to save configuration file from `config` command.")?;

            if !quiet {
                if value_changed {
                    eprintln!("Property 'channelsymlinks' set to '{}'", value);
                } else {
                    eprintln!("Property 'channelsymlinks' is already set to '{}'", value);
                }
            }
        }
        None => {
            let config_file = load_config_db(paths)
                .with_context(|| "`config` command failed to load configuration data.")?;

            if !quiet {
                eprintln!(
                    "Property 'channelsymlinks' set to '{}'",
                    config_file.data.settings.create_channel_symlinks
                );
            }
        }
    };

    Ok(())
}
