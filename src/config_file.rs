use crate::utils::get_juliaupconfig_path;
use anyhow::{bail, Context, Result};
use cluFlock::{SharedFlock, FlockLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
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

    let file = match File::open(&path) {
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

    let _file_lock = SharedFlock::wait_lock(&file).unwrap();

    println!("Lock acquired! {:?}.", _file_lock);

    let reader = BufReader::new(&file);

    let v: JuliaupConfig = serde_json::from_reader(reader)
        .with_context(|| format!("Failed to parse configuration file '{}' for reading.", display))?;

    Ok(v)
}

pub fn save_config_db(file: File, file_lock: FlockLock<&File>, config_data: &JuliaupConfig) -> Result<()> {
    file.rewind()
        .with_context(|| "Failed to rewind config file for write.")?;

    serde_json::to_writer_pretty(file, &config_data)
        .with_context(|| format!("Failed to write configuration file."))?;
    Ok(())
}
