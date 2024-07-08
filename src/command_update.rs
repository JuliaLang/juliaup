use crate::config_file::{JuliaupConfig, load_config_db};
use crate::config_file::{load_mut_config_db, save_config_db, JuliaupConfigChannel};
use crate::global_paths::GlobalPaths;
use crate::jsonstructs_versionsdb::JuliaupVersionDB;
#[cfg(not(windows))]
use crate::operations::create_symlink;
use crate::operations::{garbage_collect_versions, install_from_url};
use crate::operations::{download_version_to_temp_folder, update_version_db};
use crate::versions_file::load_versions_db;
use anyhow::{anyhow, bail, Context, Result};
use std::path::PathBuf;

fn update_channel(
    config_db: &mut JuliaupConfig,
    channel: &String,
    version_db: &JuliaupVersionDB,
    ignore_non_updatable_channel: bool,
    paths: &GlobalPaths,
) -> Result<()> {
    let current_version =
        &config_db.installed_channels.get(channel).ok_or_else(|| anyhow!("Trying to get the installed version for a channel that does not exist in the config database."))?.clone();

    match current_version {
        JuliaupConfigChannel::SystemChannel { version } => {
            let should_version = version_db.available_channels.get(channel);

            if let Some(should_version) = should_version {
                if &should_version.version != version {
                    download_version_to_temp_folder(&should_version.version, config_db, version_db, paths)
                        .with_context(|| {
                            format!(
                                "Failed to install '{}' while updating channel '{}'.",
                                should_version.version, channel
                            )
                        })?;

                    config_db.installed_channels.insert(
                        channel.clone(),
                        JuliaupConfigChannel::SystemChannel {
                            version: should_version.version.clone(),
                        },
                    );

                    #[cfg(not(windows))]
                    if config_db.settings.create_channel_symlinks {
                        create_symlink(
                            &JuliaupConfigChannel::SystemChannel {
                                version: should_version.version.clone(),
                            },
                            &format!("julia-{}", channel),
                            paths,
                        )?;
                    }
                }
            } else if ignore_non_updatable_channel {
                eprintln!("Skipping update for '{}' channel, it no longer exists in the version database.", channel);
            } else {
                bail!(
                    "Failed to update '{}' because it no longer exists in the version database.",
                    channel
                );
            }
        }
        JuliaupConfigChannel::LinkedChannel {
            command: _,
            args: _,
        } => {
            if !ignore_non_updatable_channel {
                bail!(
                    "Failed to update '{}' because it is a linked channel.",
                    channel
                );
            }
        }
        JuliaupConfigChannel::DirectDownloadChannel {
            path,
            url,
            local_etag,
            server_etag,
            version,
        } => {
            // We only do this so that we use `version` on both Windows and Linux to prevent a compiler warning/error
            assert!(!version.is_empty());

            if local_etag != server_etag {
                let channel_data =
                    install_from_url(&url::Url::parse(url)?, &PathBuf::from(path), paths)?;

                config_db
                    .installed_channels
                    .insert(channel.clone(), channel_data);

                #[cfg(not(windows))]
                if config_db.settings.create_channel_symlinks {
                    create_symlink(
                        &JuliaupConfigChannel::DirectDownloadChannel {
                            path: path.clone(),
                            url: url.clone(),
                            local_etag: local_etag.clone(),
                            server_etag: server_etag.clone(),
                            version: version.clone(),
                        },
                        &channel,
                        paths,
                    )?;
                }
            }
        }
    }

    Ok(())
}

pub fn run_command_update(channel: Option<String>, paths: &GlobalPaths) -> Result<()> {
    update_version_db(paths).with_context(|| "Failed to update versions db.")?;

    let version_db =
        load_versions_db(paths).with_context(|| "`update` command failed to load versions db.")?;

    let channels_to_update;

    {
        let config_file = load_config_db(paths)
            .with_context(|| "`update` command failed to load configuration data.")?;

        

        channels_to_update = match channel {
            None => {
                config_file
                    .data
                    .installed_channels                
                    .iter()
                    .filter_map(|i| {
                        match i.1 {
                            JuliaupConfigChannel::SystemChannel {version } => Some((i.0, version)),
                            JuliaupConfigChannel::LinkedChannel { command, args } => None,
                        }
                    })
                    .collect::<Vec<_>>()

                // for (k, _) in config_file.data.installed_channels.clone() {
                //     update_channel(&mut config_file.data, &k, &version_db, true, paths)?;
                // }
            }
            Some(channel) => {
                if !config_file.data.installed_channels.contains_key(&channel) {
                    bail!(
                        "'{}' cannot be updated because it is currently not installed.",
                        channel
                    );
                }

                config_file
                    .data
                    .installed_channels                
                    .iter()
                    .filter(|i| i.0==&channel)
                    .filter_map(|i| {
                        match i.1 {
                            JuliaupConfigChannel::SystemChannel {version } => Some((i.0, version)),
                            JuliaupConfigChannel::LinkedChannel { command, args } => None,
                        }
                    })
                    .collect::<Vec<_>>()
            }
        };
    }

    for (k,v) in channels_to_update {
        update_channel(&mut config_file.data, &channel, &version_db, false, paths)?;
    }

    garbage_collect_versions(&mut config_file.data, paths)?;

    save_config_db(&mut config_file)
        .with_context(|| "`update` command failed to save configuration db.")?;

    Ok(())
}
