use crate::config_file::JuliaupConfig;
use crate::config_file::{load_mut_config_db, save_config_db, JuliaupConfigChannel};
use crate::global_paths::GlobalPaths;
use crate::jsonstructs_versionsdb::JuliaupVersionDB;
#[cfg(not(windows))]
use crate::operations::create_symlink;
use crate::operations::{garbage_collect_versions, install_from_url};
use crate::operations::{install_version, update_version_db};
use crate::utils::{print_juliaup_style, JuliaupMessageType};
use crate::versions_file::load_versions_db;
use anyhow::{anyhow, bail, Context, Result};
use std::path::PathBuf;

fn resolve_channel_alias(config_db: &JuliaupConfig, channel_name: &str) -> Result<String> {
    match config_db.installed_channels.get(channel_name) {
        Some(JuliaupConfigChannel::AliasChannel { target, .. }) => Ok(target.to_string()),
        Some(_) => Ok(channel_name.to_string()),
        None => bail!("Channel '{}' not found", channel_name),
    }
}

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
        JuliaupConfigChannel::DirectDownloadChannel {
            path,
            url,
            local_etag,
            server_etag,
            version,
            binary_path,
        } => {
            if local_etag != server_etag {
                // We only do this so that we use `version` on both Windows and Linux to prevent a compiler warning/error
                if version.is_empty() {
                    eprintln!(
                        "Channel {channel} version is empty, you may need to manually codesign this channel if you trust the contents of this pull request."
                    );
                }
                print_juliaup_style(
                    "Updating",
                    &format!("channel {channel}"),
                    JuliaupMessageType::Progress,
                );

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
                            binary_path: binary_path.clone(),
                        },
                        channel,
                        paths,
                    )?;
                }
            }
        }
        JuliaupConfigChannel::SystemChannel { version } => {
            let should_version = version_db.available_channels.get(channel);

            if let Some(should_version) = should_version {
                if &should_version.version != version {
                    print_juliaup_style(
                        "Updating",
                        &format!("channel {}", channel),
                        JuliaupMessageType::Progress,
                    );

                    install_version(&should_version.version, config_db, version_db, paths)
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
        JuliaupConfigChannel::LinkedChannel { .. } => {
            if !ignore_non_updatable_channel {
                bail!(
                    "Failed to update '{}' because it is a linked channel.",
                    channel
                );
            }
        }
        JuliaupConfigChannel::AliasChannel { .. } => {
            unreachable!("Alias channels should be resolved before calling update_channel. Please submit a bug report.");
        }
    }

    Ok(())
}

pub fn run_command_update(channel: &Option<String>, paths: &GlobalPaths) -> Result<()> {
    update_version_db(channel, paths).with_context(|| "Failed to update versions db.")?;

    let version_db =
        load_versions_db(paths).with_context(|| "`update` command failed to load versions db.")?;

    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`update` command failed to load configuration data.")?;

    match channel {
        None => {
            for (k, v) in config_file.data.installed_channels.clone() {
                // Skip alias channels - they don't need to be updated directly
                // since they point to other channels that will be updated
                if let JuliaupConfigChannel::AliasChannel { .. } = v {
                    continue;
                }
                if let Err(e) = update_channel(&mut config_file.data, &k, &version_db, true, paths)
                {
                    print_juliaup_style(
                        "Failed",
                        &format!("to update {k}. {e}"),
                        JuliaupMessageType::Error,
                    );
                }
            }
        }
        Some(channel) => {
            if !config_file.data.installed_channels.contains_key(channel) {
                bail!(
                    "'{}' cannot be updated because it is currently not installed.",
                    channel
                );
            }

            // Resolve any aliases to get the actual target channel
            let resolved_channel = resolve_channel_alias(&config_file.data, channel)?;

            update_channel(
                &mut config_file.data,
                &resolved_channel,
                &version_db,
                false,
                paths,
            )?;
        }
    };

    garbage_collect_versions(false, &mut config_file.data, paths)?;

    save_config_db(&mut config_file)
        .with_context(|| "`update` command failed to save configuration db.")?;

    Ok(())
}
