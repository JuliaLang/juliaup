#[cfg(feature = "selfupdate")]
use anyhow::Result;

#[cfg(feature = "selfupdate")]
pub fn run_command_config_modifypath(value: Option<bool>) -> Result<()> {
    use crate::operations::{add_binfolder_to_path_in_shell_scripts, remove_binfolder_from_path_in_shell_scripts};
    use crate::config_file::{load_mut_config_db, save_config_db, load_config_db};
    use anyhow::{Context,anyhow};


    match value {
        Some(value) => {
            let mut config_file = load_mut_config_db()
                .with_context(|| "`config` command failed to load configuration data.")?;
    
            let executable_path = config_file.data.clone().self_install_location.ok_or(anyhow!("Trying to configure PATH modifications but the config file is missing the field SelfInstallLocation."))?;

            let mut value_changed = false;

            if value != config_file.data.settings.modify_path {
                config_file.data.settings.modify_path = value;

                value_changed = true;
            }

            if value {
                add_binfolder_to_path_in_shell_scripts(&executable_path).unwrap();
            }
            else {
                remove_binfolder_from_path_in_shell_scripts().unwrap();
            }

            save_config_db(&mut config_file)
                .with_context(|| "Failed to save configuration file from `config` command.")?;

            if value_changed {
                eprintln!("Property 'modifypath' set to '{}'", value);
            }
            else {
                eprintln!("Property 'modifypath' is already set to '{}'", value);
            }
        },
        None => {
            let config_data = load_config_db()
                .with_context(|| "`config` command failed to load configuration data.")?;

            eprintln!("Property 'modifypath' set to '{}'", config_data.settings.modify_path);
        }
    };

    Ok(())
}
