use crate::config_file::{load_mut_config_db, save_config_db, load_config_db};
use crate::operations::{create_symlink, remove_symlink};
use anyhow::{bail, Context, Result};

#[cfg(not(target_os = "windows"))]
pub fn run_command_config_symlinks(value: Option<bool>) -> Result<()> {
    match value {
        Some(value) => {
            let mut config_file = load_mut_config_db()
                .with_context(|| "`config` command failed to load configuration data.")?;
    
            let mut value_changed = false;

            if std::env::consts::OS == "windows" {
                bail!("Symlinks not supported on Windows.");
            }

            if value != config_file.data.create_symlinks {
                config_file.data.create_symlinks = value;
                value_changed = true;

                for (channel_name, channel) in &config_file.data.installed_channels {
                    if value {
                        create_symlink(channel, &format!("julia-{}", channel_name))?;
                    }
                    else {
                        remove_symlink(&format!("julia-{}", channel_name))?;
                    }
                }
            }

            save_config_db(config_file)
                .with_context(|| "Failed to save configuration file from `config` command.")?;

            if value_changed {
                eprintln!("Property 'symlinks' set to '{}'", value);
            }
            else {
                eprintln!("Property 'symlinks' is already set to '{}'", value);
            }
        },
        None => {
            let config_data = load_config_db()
                .with_context(|| "`config` command failed to load configuration data.")?;

            eprintln!("Property 'symlinks' set to '{}'", config_data.create_symlinks);
        }
    };

    Ok(())
}
