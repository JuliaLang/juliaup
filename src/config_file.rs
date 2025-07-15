use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use cluFlock::{ExclusiveFlock, FlockLock, SharedFlock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, ErrorKind, Seek, SeekFrom};

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
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct JuliaupConfigExcutionAlias {
    #[serde(rename = "Target")]
    pub target: String
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum JuliaupConfigApplication {
    DevedApplication {
        #[serde(rename = "Path")]
        path: String,
        #[serde(rename = "JuliaVersion")]
        julia_version: String,
        #[serde(rename = "JuliaDepot")]
        julia_depot: String,
        #[serde(rename = "ExecutionAliases")]
        execution_aliases: HashMap<String, JuliaupConfigExcutionAlias>
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
}

impl Default for JuliaupConfigSettings {
    fn default() -> Self {
        JuliaupConfigSettings {
            create_channel_symlinks: false,
            versionsdb_update_interval: default_versionsdb_update_interval(),
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
    #[serde(rename = "InstalledApplications", default)]
    pub installed_apps: HashMap<String, JuliaupConfigApplication>,
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

    return Ok(file_lock);
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
                installed_apps: HashMap::new(),
                overrides: Vec::new(),
                settings: JuliaupConfigSettings {
                    create_channel_symlinks: false,
                    versionsdb_update_interval: default_versionsdb_update_interval(),
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
                installed_apps: HashMap::new(),
                overrides: Vec::new(),
                settings: JuliaupConfigSettings {
                    create_channel_symlinks: false,
                    versionsdb_update_interval: default_versionsdb_update_interval(),
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
        self_file: self_file,
        #[cfg(feature = "selfupdate")]
        self_data: self_data,
    };

    Ok(result)
}

pub fn save_config_db(juliaup_config_file: &mut JuliaupConfigFile) -> Result<()> {
    juliaup_config_file
        .file
        .rewind()
        .with_context(|| "Failed to rewind config file for write.")?;

    juliaup_config_file
        .file
        .set_len(0)
        .with_context(|| "Failed to set len to 0 for config file before writing new content.")?;

    serde_json::to_writer_pretty(&juliaup_config_file.file, &juliaup_config_file.data)
        .with_context(|| "Failed to write configuration file.")?;

    juliaup_config_file
        .file
        .sync_all()
        .with_context(|| "Failed to write config data to disc.")?;

    #[cfg(feature = "selfupdate")]
    {
        juliaup_config_file
            .self_file
            .rewind()
            .with_context(|| "Failed to rewind self config file for write.")?;

        juliaup_config_file.self_file.set_len(0).with_context(|| {
            "Failed to set len to 0 for self config file before writing new content."
        })?;

        serde_json::to_writer_pretty(
            &juliaup_config_file.self_file,
            &juliaup_config_file.self_data,
        )
        .with_context(|| format!("Failed to write self configuration file."))?;

        juliaup_config_file
            .self_file
            .sync_all()
            .with_context(|| "Failed to write config data to disc.")?;
    }

    Ok(())
}
