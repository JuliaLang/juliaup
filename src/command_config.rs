use crate::config_file::{load_config_db, save_config_db};
use crate::operations::{create_symlink, remove_symlink};
use anyhow::{bail, Context, Result};

pub fn run_command_config(property: String, value: String) -> Result<()> {
    let mut config_data =
        load_config_db().with_context(|| "`config` command failed to load configuration file.")?;

    let mut value_changed = false;

    match property.as_str() {
        "symlinks" => {
            if std::env::consts::OS == "windows" {
                bail!("Symlinks not supported on Windows.");
            }

            let create_symlinks = match value.as_str() {
                "on"  => true,
                "off" => false,
                _     => bail!("Value for 'symlinks' must be either 'on' or 'off'."),
            };

            if create_symlinks != config_data.create_symlinks {
                config_data.create_symlinks = create_symlinks;
                value_changed = true;

                if create_symlinks {
                    for (channel_name, channel) in &config_data.installed_channels {
                        create_symlink(channel, &format!("julia-{}", channel_name))?;
                    }
                }
                else {
                    for (channel_name, _) in &config_data.installed_channels {
                        remove_symlink(&format!("julia-{}", channel_name))?;
                    }
                }
            }
        },
        s => bail!(format!("Unknown property '{}'.", s)),
    };

    save_config_db(&config_data)?;

    if value_changed {
        eprintln!("Property '{}' set to '{}'", property, value);
    }
    else {
        eprintln!("Property '{}' is already set to '{}'", property, value);
    }

    Ok(())
}
