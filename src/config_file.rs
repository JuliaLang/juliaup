use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use cluFlock::{ExclusiveFlock, FlockLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, ErrorKind, Seek};
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
    pub config_path: std::path::PathBuf,
    pub lock: FlockLock<File>,
    pub data: JuliaupConfig,
    #[cfg(feature = "selfupdate")]
    pub self_config_path: std::path::PathBuf,
    #[cfg(feature = "selfupdate")]
    pub self_data: JuliaupSelfConfig,
}

pub struct JuliaupReadonlyConfigFile {
    pub data: JuliaupConfig,
    #[cfg(feature = "selfupdate")]
    pub self_data: JuliaupSelfConfig,
}


pub fn load_config_db(paths: &GlobalPaths) -> Result<JuliaupReadonlyConfigFile> {
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

    Ok(JuliaupReadonlyConfigFile {
        data: v,
        #[cfg(feature = "selfupdate")]
        self_data: selfconfig,
    })
}

pub fn load_mut_config_db(paths: &GlobalPaths) -> Result<JuliaupConfigFile> {
    std::fs::create_dir_all(&paths.juliauphome)
        .with_context(|| "Could not create juliaup home folder.")?;

    // Open or create the config file and acquire an exclusive lock
    let config_file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&paths.juliaupconfig)
    {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("Could not open config file: {}.", e)),
    };

    // Acquire exclusive lock on the config file
    let mut file_lock = match ExclusiveFlock::try_lock(config_file) {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!(
                "Juliaup configuration is locked by another process, waiting for it to unlock."
            );
            ExclusiveFlock::wait_lock(e.into()).unwrap()
        }
    };

    // Read the configuration
    let data = {
        file_lock.rewind()
            .with_context(|| "Failed to rewind config file for reading.")?;
        
        let metadata = file_lock.metadata()
            .with_context(|| "Failed to get config file metadata.")?;
        
        if metadata.len() == 0 {
            // File is empty, create default config
            JuliaupConfig {
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
            }
        } else {
            // Read existing config
            let reader = BufReader::new(&*file_lock);
            serde_json::from_reader(reader)
                .with_context(|| "Failed to parse configuration file.")?
        }
    };

    #[cfg(feature = "selfupdate")]
    let self_data: JuliaupSelfConfig;
    #[cfg(feature = "selfupdate")]
    {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open(&paths.juliaupselfconfig)
            .with_context(|| "Failed to open juliaup self config file.")?;

        let reader = BufReader::new(&file);

        self_data = serde_json::from_reader(reader).with_context(|| {
            format!(
                "Failed to parse self configuration file '{:?}' for reading.",
                paths.juliaupselfconfig
            )
        })?
    }

    let result = JuliaupConfigFile {
        config_path: paths.juliaupconfig.clone(),
        lock: file_lock,
        data,
        #[cfg(feature = "selfupdate")]
        self_config_path: paths.juliaupselfconfig.clone(),
        #[cfg(feature = "selfupdate")]
        self_data,
    };

    Ok(result)
}

pub fn save_config_db(juliaup_config_file: &mut JuliaupConfigFile) -> Result<()> {
    // Write main config file atomically
    let parent_dir = juliaup_config_file.config_path.parent()
        .ok_or_else(|| anyhow!("Config path has no parent directory"))?;
    
    let temp_file = NamedTempFile::new_in(parent_dir)
        .with_context(|| "Failed to create temp file for config.")?;
    
    serde_json::to_writer_pretty(&temp_file, &juliaup_config_file.data)
        .with_context(|| "Failed to write configuration to temp file.")?;
    
    temp_file.persist(&juliaup_config_file.config_path)
        .with_context(|| "Failed to atomically replace configuration file.")?;

    #[cfg(feature = "selfupdate")]
    {
        let self_parent_dir = juliaup_config_file.self_config_path.parent()
            .ok_or_else(|| anyhow!("Self config path has no parent directory"))?;
        
        let self_temp_file = NamedTempFile::new_in(self_parent_dir)
            .with_context(|| "Failed to create temp file for self config.")?;
        
        serde_json::to_writer_pretty(&self_temp_file, &juliaup_config_file.self_data)
            .with_context(|| "Failed to write self configuration to temp file.")?;
        
        self_temp_file.persist(&juliaup_config_file.self_config_path)
            .with_context(|| "Failed to atomically replace self configuration file.")?;
    }

    Ok(())
}
