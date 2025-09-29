use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
#[allow(dead_code)]
pub struct JuliaupVersionDBVersion {
    #[serde(rename = "UrlPath")]
    pub url_path: String,
}

#[derive(Serialize, Deserialize)]
#[allow(dead_code)]
pub struct JuliaupVersionDBChannel {
    #[serde(rename = "Version")]
    pub version: String,
}

#[derive(Serialize, Deserialize)]
#[allow(dead_code)]
pub struct JuliaupVersionDB {
    #[serde(rename = "AvailableVersions")]
    pub available_versions: HashMap<String, JuliaupVersionDBVersion>,
    #[serde(rename = "AvailableChannels")]
    pub available_channels: HashMap<String, JuliaupVersionDBChannel>,
    #[serde(rename = "Version")]
    pub version: String,
}
