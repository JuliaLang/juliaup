use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct JuliaupVersionDBVersion {
    #[serde(rename = "UrlPath")]
    pub url_path: String,
    #[serde(rename = "Nightly", default)]
    pub nightly: bool,
}

#[derive(Serialize, Deserialize)]
pub struct JuliaupVersionDBChannel {
    #[serde(rename = "Version")]
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct JuliaupVersionDB {
    #[serde(rename = "AvailableVersions")]
    pub available_versions: HashMap<String,JuliaupVersionDBVersion>,
    #[serde(rename = "AvailableChannels")]
    pub available_channels: HashMap<String,JuliaupVersionDBChannel>
}
