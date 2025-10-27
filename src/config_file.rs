use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use cluFlock::{ExclusiveFlock, FlockLock, SharedFlock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, ErrorKind, Seek, SeekFrom, Write};
use std::mem;
use tempfile::NamedTempFile;

use crate::global_paths::GlobalPaths;

fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

fn default_versionsdb_update_interval() -> i64 {
    1440
}

fn is_default_versionsdb_update_interval(i: &i64) -> bool {
    *i == default_versionsdb_update_interval()
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct JuliaupConfigVersion {
    #[serde(rename = "Path")]
    pub path: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum JuliaupConfigChannel {
    DirectDownloadChannel {
        #[serde(rename = "Path")]
        path: String,
        #[serde(rename = "Url")]
        url: String,
        #[serde(rename = "LocalETag")]
        local_etag: String,
        #[serde(rename = "ServerETag")]
        server_etag: String,
        #[serde(rename = "Version")]
        version: String,
    },
    SystemChannel {
        #[serde(rename = "Version")]
        version: String,
    },
    LinkedChannel {
        #[serde(rename = "Command")]
        command: String,
        #[serde(rename = "Args")]
        args: Option<Vec<String>>,
    },
    AliasChannel {
        #[serde(rename = "Target")]
        target: String,
        #[serde(rename = "Args")]
        args: Option<Vec<String>>,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct JuliaupConfigSettings {
    #[serde(
        rename = "CreateChannelSymlinks",
        default,
        skip_serializing_if = "is_default"
    )]
    pub create_channel_symlinks: bool,
    #[serde(
        rename = "VersionsDbUpdateInterval",
        default = "default_versionsdb_update_interval",
        skip_serializing_if = "is_default_versionsdb_update_interval"
    )]
    pub versionsdb_update_interval: i64,
    #[serde(
        rename = "AutoInstallChannels",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub auto_install_channels: Option<bool>,
}

impl Default for JuliaupConfigSettings {
    fn default() -> Self {
        JuliaupConfigSettings {
            create_channel_symlinks: false,
            versionsdb_update_interval: default_versionsdb_update_interval(),
            auto_install_channels: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct JuliaupOverride {
    #[serde(rename = "Path")]
    pub path: String,
    #[serde(rename = "Channel")]
    pub channel: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct JuliaupConfig {
    #[serde(rename = "Default")]
    pub default: Option<String>,
    #[serde(rename = "InstalledVersions")]
    pub installed_versions: HashMap<String, JuliaupConfigVersion>,
    #[serde(rename = "InstalledChannels")]
    pub installed_channels: HashMap<String, JuliaupConfigChannel>,
    #[serde(rename = "Settings", default)]
    pub settings: JuliaupConfigSettings,
    #[serde(rename = "Overrides", default)]
    pub overrides: Vec<JuliaupOverride>,
    #[serde(
        rename = "LastVersionDbUpdate",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_version_db_update: Option<DateTime<Utc>>,
}

#[cfg(feature = "selfupdate")]
#[derive(Serialize, Deserialize, Clone)]
pub struct JuliaupSelfConfig {
    #[serde(
        rename = "BackgroundSelfUpdateInterval",
        skip_serializing_if = "Option::is_none"
    )]
    pub background_selfupdate_interval: Option<i64>,
    #[serde(
        rename = "StartupSelfUpdateInterval",
        skip_serializing_if = "Option::is_none"
    )]
    pub startup_selfupdate_interval: Option<i64>,
    #[serde(rename = "ModifyPath", default, skip_serializing_if = "is_default")]
    pub modify_path: bool,
    #[serde(rename = "JuliaupChannel", skip_serializing_if = "Option::is_none")]
    pub juliaup_channel: Option<String>,
    #[serde(rename = "LastSelfUpdate", skip_serializing_if = "Option::is_none")]
    pub last_selfupdate: Option<DateTime<Utc>>,
}

pub struct JuliaupConfigFile {
    pub file: File,
    pub lock: FlockLock<File>,
    pub data: JuliaupConfig,
    #[cfg(feature = "selfupdate")]
    pub self_file: File,
    #[cfg(feature = "selfupdate")]
    pub self_data: JuliaupSelfConfig,
}

pub struct JuliaupReadonlyConfigFile {
    pub data: JuliaupConfig,
    #[cfg(feature = "selfupdate")]
    pub self_data: JuliaupSelfConfig,
}

pub fn get_read_lock(paths: &GlobalPaths) -> Result<FlockLock<File>> {
    std::fs::create_dir_all(&paths.juliauphome)
        .with_context(|| "Could not create juliaup home folder.")?;

    let lock_file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&paths.lockfile)
    {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("Could not create lockfile: {}.", e)),
    };

    let file_lock = match SharedFlock::try_lock(lock_file) {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!(
                "Juliaup configuration is locked by another process, waiting for it to unlock."
            );

            SharedFlock::wait_lock(e.into()).unwrap()
        }
    };

    Ok(file_lock)
}

pub fn load_config_db(
    paths: &GlobalPaths,
    existing_lock: Option<&FlockLock<File>>,
) -> Result<JuliaupReadonlyConfigFile> {
    let mut file_lock: Option<FlockLock<File>> = None;

    if existing_lock.is_none() {
        file_lock = Some(get_read_lock(paths)?);
    }

    let v = match std::fs::OpenOptions::new()
        .read(true)
        .open(&paths.juliaupconfig)
    {
        Ok(file) => {
            let reader = BufReader::new(&file);

            serde_json::from_reader(reader).with_context(|| {
                format!(
                    "Failed to parse configuration file '{:?}' for reading.",
                    paths.juliaupconfig
                )
            })?
        }
        Err(error) => match error.kind() {
            ErrorKind::NotFound => JuliaupConfig {
                default: None,
                installed_versions: HashMap::new(),
                installed_channels: HashMap::new(),
                overrides: Vec::new(),
                settings: JuliaupConfigSettings {
                    create_channel_symlinks: false,
                    versionsdb_update_interval: default_versionsdb_update_interval(),
                    auto_install_channels: None,
                },
                last_version_db_update: None,
            },
            other_error => {
                bail!(
                    "Problem opening the file {:?}: {:?}",
                    paths.juliaupconfig,
                    other_error
                )
            }
        },
    };

    #[cfg(feature = "selfupdate")]
    let selfconfig: JuliaupSelfConfig;
    #[cfg(feature = "selfupdate")]
    {
        selfconfig = match std::fs::OpenOptions::new()
            .read(true)
            .open(&paths.juliaupselfconfig)
        {
            Ok(file) => {
                let reader = BufReader::new(&file);

                serde_json::from_reader(reader).with_context(|| {
                    format!(
                        "Failed to parse self configuration file '{:?}' for reading.",
                        paths.juliaupselfconfig
                    )
                })?
            }
            Err(error) => bail!(
                "Could not open self configuration file {:?}: {:?}",
                paths.juliaupselfconfig,
                error
            ),
        };
    }

    if let Some(file_lock) = file_lock {
        file_lock
            .unlock()
            .with_context(|| "Failed to unlock configuration file.")?;
    }

    Ok(JuliaupReadonlyConfigFile {
        data: v,
        #[cfg(feature = "selfupdate")]
        self_data: selfconfig,
    })
}

pub fn load_mut_config_db(paths: &GlobalPaths) -> Result<JuliaupConfigFile> {
    std::fs::create_dir_all(&paths.juliauphome)
        .with_context(|| "Could not create juliaup home folder.")?;

    let lock_file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&paths.lockfile)
    {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("Could not create lockfile: {}.", e)),
    };

    let file_lock = match ExclusiveFlock::try_lock(lock_file) {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!(
                "Juliaup configuration is locked by another process, waiting for it to unlock."
            );

            ExclusiveFlock::wait_lock(e.into()).unwrap()
        }
    };

    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&paths.juliaupconfig)
        .with_context(|| "Failed to open juliaup config file.")?;

    let stream_len = file
        .seek(SeekFrom::End(0))
        .with_context(|| "Failed to determine the length of the configuration file.")?;

    let data = match stream_len {
        0 => {
            let new_config = JuliaupConfig {
                default: None,
                installed_versions: HashMap::new(),
                installed_channels: HashMap::new(),
                overrides: Vec::new(),
                settings: JuliaupConfigSettings {
                    create_channel_symlinks: false,
                    versionsdb_update_interval: default_versionsdb_update_interval(),
                    auto_install_channels: None,
                },
                last_version_db_update: None,
            };

            serde_json::to_writer_pretty(&file, &new_config)
                .with_context(|| "Failed to write configuration file.")?;

            file.sync_all()
                .with_context(|| "Failed to write configuration data to disc.")?;

            file.rewind()
                .with_context(|| "Failed to rewind config file after initial write of data.")?;

            new_config
        }
        _ => {
            file.rewind()
                .with_context(|| "Failed to rewind existing config file.")?;

            let reader = BufReader::new(&file);

            serde_json::from_reader(reader)
                .with_context(|| "Failed to parse configuration file.")?
        }
    };

    #[cfg(feature = "selfupdate")]
    let self_file: File;
    #[cfg(feature = "selfupdate")]
    let self_data: JuliaupSelfConfig;
    #[cfg(feature = "selfupdate")]
    {
        self_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&paths.juliaupselfconfig)
            .with_context(|| "Failed to open juliaup config file.")?;

        let reader = BufReader::new(&self_file);

        self_data = serde_json::from_reader(reader).with_context(|| {
            format!(
                "Failed to parse self configuration file '{:?}' for reading.",
                paths.juliaupselfconfig
            )
        })?
    }

    let result = JuliaupConfigFile {
        file,
        lock: file_lock,
        data,
        #[cfg(feature = "selfupdate")]
        self_file,
        #[cfg(feature = "selfupdate")]
        self_data,
    };

    Ok(result)
}

pub fn save_config_db(juliaup_config_file: &mut JuliaupConfigFile) -> Result<()> {
    // Get the path to the config file from the file descriptor
    // We need to use a temporary file and atomic rename to avoid corruption
    // in case of disk quota or other I/O errors

    // Get the paths from the global state
    let paths = crate::global_paths::get_paths()?;

    // Write to a temporary file in the same directory as the target file
    // This ensures we're on the same filesystem for atomic rename
    let config_dir = paths
        .juliaupconfig
        .parent()
        .ok_or_else(|| anyhow!("Config file path has no parent directory"))?;

    let mut temp_file = NamedTempFile::new_in(config_dir)
        .with_context(|| "Failed to create temporary config file.")?;

    // Write the configuration data to the temporary file
    serde_json::to_writer_pretty(&mut temp_file, &juliaup_config_file.data)
        .with_context(|| "Failed to write configuration data to temporary file.")?;

    // Ensure all data is written to disk before persisting
    temp_file
        .flush()
        .with_context(|| "Failed to flush configuration data to temporary file.")?;

    temp_file
        .as_file()
        .sync_all()
        .with_context(|| "Failed to sync configuration data to disc.")?;

    // On Windows, we must close the file before replacing it
    // On Unix, the file can remain open during atomic rename
    #[cfg(target_os = "windows")]
    {
        // Create a dummy file to replace the handle we need to close
        let dummy = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(config_dir.join(".dummy"))
            .with_context(|| "Failed to create dummy file handle.")?;

        // Replace the file handle with the dummy, dropping the old handle
        let _ = mem::replace(&mut juliaup_config_file.file, dummy);
    }

    // Atomically replace the old config file with the new one
    temp_file
        .persist(&paths.juliaupconfig)
        .with_context(|| "Failed to persist configuration file.")?;

    // Reopen the file to update our file handle (Windows only, since we closed it)
    #[cfg(target_os = "windows")]
    {
        juliaup_config_file.file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&paths.juliaupconfig)
            .with_context(|| "Failed to reopen configuration file after save.")?;
    }

    #[cfg(feature = "selfupdate")]
    {
        // Use the same atomic write pattern for the self config file
        let self_config_dir = paths
            .juliaupselfconfig
            .parent()
            .ok_or_else(|| anyhow!("Self config file path has no parent directory"))?;

        let mut temp_self_file = NamedTempFile::new_in(self_config_dir)
            .with_context(|| "Failed to create temporary self config file.")?;

        serde_json::to_writer_pretty(&mut temp_self_file, &juliaup_config_file.self_data)
            .with_context(|| "Failed to write self configuration data to temporary file.")?;

        temp_self_file
            .flush()
            .with_context(|| "Failed to flush self configuration data to temporary file.")?;

        temp_self_file
            .as_file()
            .sync_all()
            .with_context(|| "Failed to sync self configuration data to disc.")?;

        // On Windows, we must close the file before replacing it
        #[cfg(target_os = "windows")]
        {
            // Create a dummy file to replace the handle we need to close
            let dummy = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(self_config_dir.join(".dummy_self"))
                .with_context(|| "Failed to create dummy self config file handle.")?;

            // Replace the file handle with the dummy, dropping the old handle
            let _ = mem::replace(&mut juliaup_config_file.self_file, dummy);
        }

        temp_self_file
            .persist(&paths.juliaupselfconfig)
            .with_context(|| "Failed to persist self configuration file.")?;

        // Reopen the self config file (Windows only, since we closed it)
        #[cfg(target_os = "windows")]
        {
            juliaup_config_file.self_file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&paths.juliaupselfconfig)
                .with_context(|| "Failed to reopen self configuration file after save.")?;
        }
    }

    Ok(())
}
