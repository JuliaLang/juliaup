use crate::config_file::load_config_db;
use crate::versions_file::load_versions_db;
use anyhow::{Context, Result,anyhow};
use crate::config_file::JuliaupConfigChannel;

pub fn run_command_status() -> Result<()> {
    let config_data = load_config_db()
        .with_context(|| "`status` command failed to load configuration file.")?;

    let versiondb_data = load_versions_db()
        .with_context(|| "`status` command failed to load versions db.")?;

    println!("Installed Julia channels (default marked with *):");

    for (key,value) in config_data.installed_channels {
        if key==config_data.default {
            print!("  * ");
        }
        else {
            print!("    ")
        }
        
        print!(" {}", key);

        match value {
            JuliaupConfigChannel::SystemChannel {version} => {
                match versiondb_data.available_channels.get(&key) {
                    Some(channel) => {
                        if channel.version != version {
                            print!(" (Update from {} to {} available)", version, channel.version);
                        }
                    },
                    None => return Err(anyhow!("The channel '{}' is configured as a system channel, but no such channel exists in the versions database.", key))
                }
            },
            JuliaupConfigChannel::LinkedChannel {command} => print!(" (linked to `{}`)", command)
        }

        println!();
    }

    Ok(())
}
