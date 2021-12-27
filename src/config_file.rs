use crate::utils::{get_juliaupconfig_path, get_juliaupconfig_lockfile_path, get_juliaup_home_path};
use anyhow::{anyhow, bail, Context, Result};
use cluFlock::{SharedFlock, FlockLock, ExclusiveFlock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, ErrorKind, Seek, SeekFrom};

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
pub struct JuliaupConfig {
    #[serde(rename = "Default")]
    pub default: Option<String>,
    #[serde(rename = "InstalledVersions")]
    pub installed_versions: HashMap<String, JuliaupConfigVersion>,
    #[serde(rename = "InstalledChannels")]
    pub installed_channels: HashMap<String, JuliaupConfigChannel>,
    #[serde(rename = "JuliaupChannel", skip_serializing_if = "Option::is_none")]
    pub juliaup_channel: Option<String>,
    #[serde(rename = "CreateSymlinks", default)]
    pub create_symlinks: bool,
}

pub struct JuliaupConfigFile {
    pub file: File,
    pub lock: FlockLock<File>,
    pub data: JuliaupConfig
}

pub fn load_config_db() -> Result<JuliaupConfig> {
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

    let file = match std::fs::OpenOptions::new().read(true).open(&path) {
        Ok(file) => file,
        Err(error) =>  match error.kind() {
            ErrorKind::NotFound => {
                return Ok(JuliaupConfig {
                    default: None,
                    installed_versions: HashMap::new(),
                    installed_channels: HashMap::new(),
                    juliaup_channel: None,
                    create_symlinks: false,
                })
            },
            other_error => {
                bail!("Problem opening the file {}: {:?}", display, other_error)
            }
        },
    };

    let reader = BufReader::new(&file);

    let v: JuliaupConfig = serde_json::from_reader(reader)
        .with_context(|| format!("Failed to parse configuration file '{}' for reading.", display))?;

    file_lock.unlock()
        .with_context(|| "Failed to unlock configuration file.")?;

    Ok(v)
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
                juliaup_channel: None,
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

    let result = JuliaupConfigFile {
        file,
        lock: file_lock,
        data
    };

    Ok(result)
}

pub fn save_config_db(mut juliaup_config_file: JuliaupConfigFile) -> Result<()> {
    juliaup_config_file.file.rewind()
        .with_context(|| "Failed to rewind config file for write.")?;

    juliaup_config_file.file.set_len(0)
        .with_context(|| "Failed to set len to 0 for config file before writing new content.")?;

    serde_json::to_writer_pretty(&juliaup_config_file.file, &juliaup_config_file.data)
        .with_context(|| format!("Failed to write configuration file."))?;

    juliaup_config_file.file.sync_all()
        .with_context(|| "Failed to write config data to disc.")?;

    juliaup_config_file.lock.unlock()
        .with_context(|| "Failed to unlock.")?;

    Ok(())
}
