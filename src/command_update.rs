use crate::config_file::{JuliaupConfigChannel, load_mut_config_db, save_config_db};
use crate::global_paths::GlobalPaths;
use crate::operations::{install_version};
use crate::jsonstructs_versionsdb::JuliaupVersionDB;
use crate::config_file::JuliaupConfig;
use crate::operations::garbage_collect_versions;
use crate::versions_file::load_versions_db;
use anyhow::{Context, Result,anyhow,bail};
#[cfg(not(windows))]
use crate::operations::create_symlink;

fn update_channel(config_db: &mut JuliaupConfig, channel: &String, version_db: &JuliaupVersionDB, ignore_linked_channel: bool, paths: &GlobalPaths) -> Result<()> {    
    let current_version = 
        config_db.installed_channels.get(channel).ok_or(anyhow!("asdf"))?;

    match current_version {
        JuliaupConfigChannel::SystemChannel {version} => {
            let should_version = &version_db.available_channels
                .get(channel)
                .ok_or(anyhow!("asdf"))?
                .version;
            let nightly = version_db.available_versions
                .get(should_version)
                .ok_or(anyhow!("asdf"))?
                .nightly;

            if nightly || should_version != version {
                install_version(should_version, config_db, version_db, paths)
                    .with_context(|| format!("Failed to install '{}' while updating channel '{}'.", should_version, channel))?;
        
                config_db.installed_channels.insert(
                    channel.clone(),
                    JuliaupConfigChannel::SystemChannel {
                        version: should_version.clone(),
                    },
                );

                #[cfg(not(windows))]
                if config_db.settings.create_channel_symlinks {
                    create_symlink(
                        &JuliaupConfigChannel::SystemChannel {
                            version: should_version.clone(),
                        },
                        &format!("julia-{}", channel),
                        paths,
                    )?;
                }
            }
        },
        JuliaupConfigChannel::LinkedChannel {command: _, args: _} => if !ignore_linked_channel {
            bail!("Failed to update '{}' because it is a linked channel.", channel)
        } else {()}
    }

    Ok(())
}

pub fn run_command_update(channel: Option<String>, paths: &GlobalPaths) -> Result<()> {
    let version_db =
        load_versions_db().with_context(|| "`update` command failed to load versions db.")?;

    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`update` command failed to load configuration data.")?;

    match channel {
        None => {
            for (k,_) in config_file.data.installed_channels.clone() {
                update_channel(&mut config_file.data, &k, &version_db, true, paths)?;
            }

        },
        Some(channel) => {
            if !config_file.data.installed_channels.contains_key(&channel) {
                bail!("'{}' cannot be updated because it is currently not installed.", channel);
            }

            update_channel(&mut config_file.data, &channel, &version_db, false, paths)?;
        }
    };

    garbage_collect_versions(&mut config_file.data, paths)?;

    save_config_db(&mut config_file)
        .with_context(|| "`update` command failed to save configuration db.")?;

    Ok(())
}
