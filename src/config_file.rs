use crate::utils::{get_juliaupconfig_path, get_juliaup_home_path};
use anyhow::{bail, Context, Result};
use cluFlock::{SharedFlock, FlockLock, ExclusiveFlock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, ErrorKind, Seek};

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
    pub juliaup_channel: Option<String>
}

pub fn load_config_db() -> Result<JuliaupConfig> {
    let path =
        get_juliaupconfig_path().with_context(|| "Failed to determine configuration file path.")?;

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
                })
            },
            other_error => {
                bail!("Problem opening the file {}: {:?}", display, other_error)
            }
        },
    };

    let file_lock = match SharedFlock::try_lock(&file) {
        Ok(lock) => lock,
        Err(_e) => {
            eprintln!("Juliaup configuration is locked by another process, waiting for it to unlock.");

            SharedFlock::wait_lock(&file).unwrap()
        }
    };

    let reader = BufReader::new(&file);

    let v: JuliaupConfig = serde_json::from_reader(reader)
        .with_context(|| format!("Failed to parse configuration file '{}' for reading.", display))?;

    file_lock.unlock()
        .with_context(|| "Failed to unlock configuration file.")?;

    Ok(v)
}

pub fn open_mut_config_file() -> Result<File> {
    let path = get_juliaupconfig_path()
        .with_context(|| "Failed to determine configuration file path.")?;

    let display = path.display();

    let file = match std::fs::OpenOptions::new().read(true).write(true).open(&path) {
        Ok(file) => file,
        Err(error) =>  match error.kind() {
            ErrorKind::NotFound => {
                let new_file_path = get_juliaup_home_path()?.join("~juliaup.json");

                {
                    let new_file = OpenOptions::new().create_new(true).open(&new_file_path)?;

                    let new_config = JuliaupConfig {
                        default: None,
                        installed_versions: HashMap::new(),
                        installed_channels: HashMap::new(),
                        juliaup_channel: None,
                    };

                    serde_json::to_writer_pretty(&new_file, &new_config)
                        .with_context(|| format!("Failed to write configuration file.")).unwrap();

                    new_file.sync_all()
                        .with_context(|| "Failed to write configuration data to disc.").unwrap();
                }

                std::fs::rename(&new_file_path, &path)?;

                std::fs::OpenOptions::new().read(true).write(true).open(&path)?
            },
            other_error => {
                bail!("Problem opening the file {}: {:?}", display, other_error)
            }
        },
    };

    Ok(file)
}

pub fn load_mut_config_db(file: &File) -> Result<(JuliaupConfig,FlockLock<&File>)> {
    let file_lock = match ExclusiveFlock::try_lock(file) {
        Ok(lock) => lock,
        Err(_e)  => {
            eprintln!("Juliaup configuration is locked by another process, waiting for it to unlock.");

            ExclusiveFlock::wait_lock(file).unwrap()
        }
    };

    let reader = BufReader::new(file);

    let data = serde_json::from_reader(reader)
        .with_context(|| "Failed to parse configuration file.")?;

    Ok((data, file_lock))
}

pub fn save_config_db(mut file: &File, config: JuliaupConfig, _file_lock: FlockLock<&File>) -> Result<()> {
    file.rewind()
        .with_context(|| "Failed to rewind config file for write.")?;

    file.set_len(0)
        .with_context(|| "Failed to set len to 0 for config file before writing new content.")?;

    serde_json::to_writer_pretty(file, &config)
        .with_context(|| format!("Failed to write configuration file."))?;

    file.sync_all()
        .with_context(|| "Failed to write config data to disc.")?;

    Ok(())
}
