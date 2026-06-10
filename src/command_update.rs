use crate::config_file::JuliaupConfig;
use crate::config_file::{
    get_read_lock, load_config_db, load_mut_config_db, save_config_db, JuliaupConfigChannel,
};
use crate::global_paths::GlobalPaths;
use crate::jsonstructs_versionsdb::JuliaupVersionDB;
#[cfg(not(windows))]
use crate::operations::create_symlink;
use crate::operations::{
    commit_version_install, download_version_to_temp, garbage_collect_versions, install_from_url,
    is_pr_channel, update_version_db,
};
use crate::utils::{print_juliaup_style, JuliaupMessageType};
use crate::versions_file::load_versions_db;
use anyhow::{anyhow, bail, Context, Result};
use std::path::PathBuf;
use tempfile::TempDir;

fn resolve_channel_alias(config_db: &JuliaupConfig, channel_name: &str) -> Result<String> {
    match config_db.installed_channels.get(channel_name) {
        Some(JuliaupConfigChannel::AliasChannel { target, .. }) => Ok(target.to_string()),
        Some(_) => Ok(channel_name.to_string()),
        None => bail!("Channel '{}' not found", channel_name),
    }
}

/// A channel update that has been prepared (downloaded) without holding the
/// configuration lock, ready to be committed under the exclusive lock.
enum PreparedUpdate {
    /// A database (system) channel update. `downloaded` is `None` when the
    /// target version was already installed and only the channel pointer needs
    /// to move.
    System {
        channel: String,
        new_version: String,
        downloaded: Option<TempDir>,
    },
    /// A direct-download (nightly/PR) channel update. `install_from_url` has
    /// already placed the new install on disk; only the config entry remains.
    DirectDownload {
        channel: String,
        channel_data: JuliaupConfigChannel,
    },
}

impl PreparedUpdate {
    fn channel(&self) -> &str {
        match self {
            PreparedUpdate::System { channel, .. }
            | PreparedUpdate::DirectDownload { channel, .. } => channel,
        }
    }
}

/// Phase 1 (no lock held): decide whether `channel` needs updating based on a
/// configuration snapshot and, if so, perform the network download. Returns
/// `None` when the channel is already up to date or is not updatable.
fn prepare_channel_update(
    config_db: &JuliaupConfig,
    channel: &str,
    version_db: &JuliaupVersionDB,
    ignore_non_updatable_channel: bool,
    paths: &GlobalPaths,
) -> Result<Option<PreparedUpdate>> {
    let current_version = config_db.installed_channels.get(channel).ok_or_else(|| anyhow!("Trying to get the installed version for a channel that does not exist in the config database."))?;

    match current_version {
        JuliaupConfigChannel::DirectDownloadChannel {
            path,
            url,
            local_etag,
            server_etag,
            version,
            binary_path: _,
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

                let (channel_data, _used_dmg) = install_from_url(
                    &url::Url::parse(url)?,
                    &PathBuf::from(path),
                    is_pr_channel(channel),
                    paths,
                )?;

                Ok(Some(PreparedUpdate::DirectDownload {
                    channel: channel.to_string(),
                    channel_data,
                }))
            } else {
                Ok(None)
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

                    // Only download if the target version is not already installed.
                    let downloaded = if config_db
                        .installed_versions
                        .contains_key(&should_version.version)
                    {
                        None
                    } else {
                        Some(
                            download_version_to_temp(&should_version.version, version_db, paths)
                                .with_context(|| {
                                    format!(
                                        "Failed to download '{}' while updating channel '{}'.",
                                        should_version.version, channel
                                    )
                                })?,
                        )
                    };

                    Ok(Some(PreparedUpdate::System {
                        channel: channel.to_string(),
                        new_version: should_version.version.clone(),
                        downloaded,
                    }))
                } else {
                    Ok(None)
                }
            } else if ignore_non_updatable_channel {
                eprintln!("Skipping update for '{}' channel, it no longer exists in the version database.", channel);
                Ok(None)
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
            Ok(None)
        }
        JuliaupConfigChannel::AliasChannel { .. } => {
            unreachable!("Alias channels should be resolved before calling prepare_channel_update. Please submit a bug report.");
        }
    }
}

/// Phase 2 (exclusive lock held): commit a previously prepared update into the
/// configuration. If the channel was removed concurrently, the prepared update
/// is discarded.
fn commit_channel_update(
    config_db: &mut JuliaupConfig,
    prepared: PreparedUpdate,
    paths: &GlobalPaths,
) -> Result<()> {
    // If the channel was removed while we were downloading, discard the update.
    if !config_db
        .installed_channels
        .contains_key(prepared.channel())
    {
        return Ok(());
    }

    match prepared {
        PreparedUpdate::DirectDownload {
            channel,
            channel_data,
        } => {
            #[cfg(not(windows))]
            if config_db.settings.create_channel_symlinks {
                create_symlink(&channel_data, &channel, paths)?;
            }

            config_db.installed_channels.insert(channel, channel_data);
        }
        PreparedUpdate::System {
            channel,
            new_version,
            downloaded,
        } => {
            if let Some(downloaded) = downloaded {
                commit_version_install(downloaded, &new_version, config_db, paths).with_context(
                    || {
                        format!(
                            "Failed to install '{}' while updating channel '{}'.",
                            new_version, channel
                        )
                    },
                )?;
            }

            config_db.installed_channels.insert(
                channel.clone(),
                JuliaupConfigChannel::SystemChannel {
                    version: new_version.clone(),
                },
            );

            #[cfg(not(windows))]
            if config_db.settings.create_channel_symlinks {
                create_symlink(
                    &JuliaupConfigChannel::SystemChannel {
                        version: new_version,
                    },
                    &format!("julia-{}", channel),
                    paths,
                )?;
            }
        }
    }

    Ok(())
}

pub fn run_command_update(channel: &Option<String>, paths: &GlobalPaths) -> Result<()> {
    update_version_db(channel, paths).with_context(|| "Failed to update versions db.")?;

    let version_db =
        load_versions_db(paths).with_context(|| "`update` command failed to load versions db.")?;

    // Phase 1: take a snapshot of the configuration under a short-lived shared
    // lock, release it, then perform all downloads with no lock held so that
    // concurrent julia/juliaup invocations are not blocked.
    let config_snapshot = {
        let file_lock = get_read_lock(paths)?;
        let config_file = load_config_db(paths, Some(&file_lock))
            .with_context(|| "`update` command failed to load configuration data.")?;
        let snapshot = config_file.data.clone();
        let (_, res) = file_lock.data_unlock();
        res.with_context(|| {
            format!(
                "Failed to unlock configuration lock file `{}`.",
                paths.lockfile.display()
            )
        })?;
        snapshot
    };

    let update_all = channel.is_none();

    let channels_to_update: Vec<String> = match channel {
        None => config_snapshot
            .installed_channels
            .iter()
            // Skip alias channels - they don't need to be updated directly
            // since they point to other channels that will be updated.
            .filter(|(_, v)| !matches!(v, JuliaupConfigChannel::AliasChannel { .. }))
            .map(|(k, _)| k.clone())
            .collect(),
        Some(channel) => {
            if !config_snapshot.installed_channels.contains_key(channel) {
                bail!(
                    "'{}' cannot be updated because it is currently not installed.",
                    channel
                );
            }
            // Resolve any aliases to get the actual target channel
            vec![resolve_channel_alias(&config_snapshot, channel)?]
        }
    };

    let mut prepared_updates = Vec::new();
    for name in channels_to_update {
        match prepare_channel_update(&config_snapshot, &name, &version_db, update_all, paths) {
            Ok(Some(prepared)) => prepared_updates.push(prepared),
            Ok(None) => {}
            Err(e) => {
                if update_all {
                    print_juliaup_style(
                        "Failed",
                        &format!("to update {name}. {e}"),
                        JuliaupMessageType::Error,
                    );
                } else {
                    return Err(e);
                }
            }
        }
    }

    // Phase 2: re-acquire the exclusive lock only to commit the prepared updates.
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`update` command failed to load configuration data.")?;

    for prepared in prepared_updates {
        let name = prepared.channel().to_string();
        if let Err(e) = commit_channel_update(&mut config_file.data, prepared, paths) {
            if update_all {
                print_juliaup_style(
                    "Failed",
                    &format!("to update {name}. {e}"),
                    JuliaupMessageType::Error,
                );
            } else {
                return Err(e);
            }
        }
    }

    garbage_collect_versions(false, &mut config_file.data, paths)?;

    save_config_db(&mut config_file, paths).with_context(|| {
        format!(
            "`update` command failed to save configuration db at `{}`.",
            paths.juliaupconfig.display()
        )
    })?;

    Ok(())
}
