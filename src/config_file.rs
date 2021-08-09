use crate::utils::get_juliaupconfig_path;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

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
    pub default: String,
    #[serde(rename = "InstalledVersions")]
    pub installed_versions: HashMap<String, JuliaupConfigVersion>,
    #[serde(rename = "InstalledChannels")]
    pub installed_channels: HashMap<String, JuliaupConfigChannel>,
}

pub fn load_config_db() -> Result<JuliaupConfig> {
    let path =
        get_juliaupconfig_path().with_context(|| "Failed to determine configuration file path.")?;

    let display = path.display();

    let file = match File::open(&path) {
        Ok(file) => file,
        // TODO Change this to only return a default if the file doesn't exist, error otherwise.
        Err(_) => {
            return Ok(JuliaupConfig {
                default: "release".to_string(),
                installed_versions: HashMap::new(),
                installed_channels: HashMap::new(),
            })
        }
    };

    let reader = BufReader::new(file);

    let v: JuliaupConfig = serde_json::from_reader(reader).with_context(|| {
        format!(
            "Failed to parse configuration file '{}' for reading.",
            display
        )
    })?;

    Ok(v)
}

pub fn save_config_db(config_data: &JuliaupConfig) -> Result<()> {
    let path =
        get_juliaupconfig_path().with_context(|| "Failed to determine configuration file path.")?;

    let display = path.display();

    let file = File::create(&path).with_context(|| {
        format!(
            "Failed to open configuration file '{}' for saving.",
            display
        )
    })?;

    serde_json::to_writer(file, &config_data)
        .with_context(|| format!("Failed to write configuration file '{}'.", display))?;
    Ok(())
}
