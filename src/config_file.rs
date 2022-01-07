use crate::utils::{get_juliaupconfig_path, get_juliaupconfig_lockfile_path, get_juliaup_home_path};
#[cfg(feature = "selfupdate")]
use crate::utils::get_juliaupselfconfig_path;
use anyhow::{anyhow, bail, Context, Result};
use cluFlock::{SharedFlock, FlockLock, ExclusiveFlock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, ErrorKind, Seek, SeekFrom};
#[cfg(feature = "selfupdate")]
use chrono::{DateTime,Utc};

fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

#[derive(Serialize, Deserialize, Clone)]
pub struct JuliaupConfigVersion {
    #[serde(rename = "Path")]
    pub path: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum JuliaupConfigChannel {
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

#[derive(Serialize, Deserialize, Clone)]
pub struct JuliaupConfigSettings {
    #[serde(rename = "CreateChannelSymlinks", default, skip_serializing_if = "is_default")]
    pub create_channel_symlinks: bool,
}

impl Default for JuliaupConfigSettings {
    fn default() -> Self { 
        JuliaupConfigSettings {
            create_channel_symlinks: false,
        }
     }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct JuliaupConfig {
    #[serde(rename = "Default")]
    pub default: Option<String>,
    #[serde(rename = "InstalledVersions")]
    pub installed_versions: HashMap<String, JuliaupConfigVersion>,
    #[serde(rename = "InstalledChannels")]
    pub installed_channels: HashMap<String, JuliaupConfigChannel>,
    #[serde(rename = "Settings", default)]
    pub settings: JuliaupConfigSettings,
}

#[cfg(feature = "selfupdate")]
#[derive(Serialize, Deserialize, Clone)]
pub struct JuliaupSelfConfig {
    #[serde(rename = "BackgroundSelfUpdateInterval", skip_serializing_if = "Option::is_none")]
    pub background_selfupdate_interval: Option<i64>,
    #[serde(rename = "StartupSelfUpdateInterval", skip_serializing_if = "Option::is_none")]
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
    pub self_data: JuliaupSelfConfig
}

pub struct JuliaupReadonlyConfigFile {
    pub data: JuliaupConfig,
    #[cfg(feature = "selfupdate")]
    pub self_data: JuliaupSelfConfig
}

pub fn load_config_db() -> Result<JuliaupReadonlyConfigFile> {
    let path =
        get_juliaupconfig_path().with_context(|| "Failed to determine configuration file path.")?;

    let lockfile_path = get_juliaupconfig_lockfile_path()
        .with_context(|| "Failed to get path for lockfile.")?;

    let juliaup_home_path = get_juliaup_home_path()
        .with_context(|| "Could not determine the juliaup home path.")?;
    
    std::fs::create_dir_all(&juliaup_home_path)
        .with_context(|| "Could not create juliaup home folder.")?;

    let lock_file = match OpenOptions::new().read(true).write(true).create(true).open(&lockfile_path) {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("Could not create lockfile: {}.", e))
    };

    let file_lock = match SharedFlock::try_lock(&lock_file) {
        Ok(lock) => lock,
        Err(_e) => {
            eprintln!("Juliaup configuration is locked by another process, waiting for it to unlock.");

            SharedFlock::wait_lock(&lock_file).unwrap()
        }
    };

    let display = path.display();

    let v = match std::fs::OpenOptions::new().read(true).open(&path) {
        Ok(file) => {
            let reader = BufReader::new(&file);

            serde_json::from_reader(reader)
                .with_context(|| format!("Failed to parse configuration file '{}' for reading.", display))?
        },
        Err(error) =>  match error.kind() {
            ErrorKind::NotFound => {
                JuliaupConfig {
                    default: None,
                    installed_versions: HashMap::new(),
                    installed_channels: HashMap::new(),
                    settings: JuliaupConfigSettings {
                        create_channel_symlinks: false,
                    },
                }
            },
            other_error => {
                bail!("Problem opening the file {}: {:?}", display, other_error)
            }
        },
    };

    #[cfg(feature = "selfupdate")]
    let selfconfig: JuliaupSelfConfig;
    #[cfg(feature = "selfupdate")]
    {
        let self_config_path = get_juliaupselfconfig_path().unwrap();

        selfconfig = match std::fs::OpenOptions::new().read(true).open(&self_config_path) {
            Ok(file) => {
                let reader = BufReader::new(&file);
    
                serde_json::from_reader(reader)
                    .with_context(|| format!("Failed to parse self configuration file '{:?}' for reading.", self_config_path))?
            },
            Err(error) => bail!("Could not open self configuration file {:?}: {:?}", self_config_path, error)
        };
    }

    file_lock.unlock()
        .with_context(|| "Failed to unlock configuration file.")?;

    Ok(JuliaupReadonlyConfigFile {
        data: v,
        #[cfg(feature = "selfupdate")]
        self_data: selfconfig,
    })
}

pub fn load_mut_config_db() -> Result<JuliaupConfigFile> {
    let path =
        get_juliaupconfig_path().with_context(|| "Failed to determine configuration file path.")?;

    let lockfile_path = get_juliaupconfig_lockfile_path()
        .with_context(|| "Failed to get path for lockfile.")?;

    let juliaup_home_path = get_juliaup_home_path()
        .with_context(|| "Could not determine the juliaup home path.")?;
    
    std::fs::create_dir_all(&juliaup_home_path)
        .with_context(|| "Could not create juliaup home folder.")?;

    let lock_file = match OpenOptions::new().read(true).write(true).create(true).open(&lockfile_path) {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("Could not create lockfile: {}.", e))
    };

    let file_lock = match ExclusiveFlock::try_lock(lock_file) {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!("Juliaup configuration is locked by another process, waiting for it to unlock.");

            ExclusiveFlock::wait_lock(e.into()).unwrap()
        }
    };

    let mut file = std::fs::OpenOptions::new().read(true).write(true).create(true).open(&path)
        .with_context(|| "Failed to open juliaup config file.")?;

    let stream_len = file.seek(SeekFrom::End(0))
        .with_context(|| "Failed to determine the length of the configuration file.")?;

    let data = match stream_len {
        0 => {
            let new_config = JuliaupConfig {
                default: None,
                installed_versions: HashMap::new(),
                installed_channels: HashMap::new(),
                settings: JuliaupConfigSettings{
                    create_channel_symlinks: false,
                },
            };

            serde_json::to_writer_pretty(&file, &new_config)
                .with_context(|| format!("Failed to write configuration file."))?;

            file.sync_all()
                .with_context(|| "Failed to write configuration data to disc.")?;
        
            file.rewind()
                .with_context(|| "Failed to rewind config file after initial write of data.")?;

            new_config
        },
        _ => {
            file.rewind()
                .with_context(|| "Failed to rewind existing config file.")?;

            let reader = BufReader::new(&file);

            let data = serde_json::from_reader(reader)
                .with_context(|| "Failed to parse configuration file.")?;

            data
        }
    };

    #[cfg(feature = "selfupdate")]
    let self_file: File;
    #[cfg(feature = "selfupdate")]
    let self_data: JuliaupSelfConfig;
    #[cfg(feature = "selfupdate")]
    {
        let self_config_path = get_juliaupselfconfig_path().unwrap();

        self_file = std::fs::OpenOptions::new().read(true).write(true).open(&self_config_path)
            .with_context(|| "Failed to open juliaup config file.")?;

        let reader = BufReader::new(&self_file);
    
        self_data = serde_json::from_reader(reader)
            .with_context(|| format!("Failed to parse self configuration file '{:?}' for reading.", self_config_path))?
    }

    let result = JuliaupConfigFile {
        file,
        lock: file_lock,
        data,
        #[cfg(feature = "selfupdate")]
        self_file: self_file,
        #[cfg(feature = "selfupdate")]
        self_data: self_data
    };

    Ok(result)
}

pub fn save_config_db(juliaup_config_file: &mut JuliaupConfigFile) -> Result<()> {
    juliaup_config_file.file.rewind()
        .with_context(|| "Failed to rewind config file for write.")?;

    juliaup_config_file.file.set_len(0)
        .with_context(|| "Failed to set len to 0 for config file before writing new content.")?;

    serde_json::to_writer_pretty(&juliaup_config_file.file, &juliaup_config_file.data)
        .with_context(|| format!("Failed to write configuration file."))?;

    juliaup_config_file.file.sync_all()
        .with_context(|| "Failed to write config data to disc.")?;

    #[cfg(feature = "selfupdate")]
    {
        juliaup_config_file.self_file.rewind()
            .with_context(|| "Failed to rewind self config file for write.")?;

        juliaup_config_file.self_file.set_len(0)
            .with_context(|| "Failed to set len to 0 for self config file before writing new content.")?;

        serde_json::to_writer_pretty(&juliaup_config_file.self_file, &juliaup_config_file.self_data)
            .with_context(|| format!("Failed to write self configuration file."))?;

        juliaup_config_file.self_file.sync_all()
            .with_context(|| "Failed to write config data to disc.")?;
    }

    Ok(())
}
