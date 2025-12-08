use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
pub struct JuliaupVersionDBSource {
    #[serde(rename = "Url")]
    pub url: String,
    #[serde(rename = "Type")]
    pub source_type: String,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum JuliaupVersionDBVersion {
    V1 {
        #[serde(rename = "UrlPath")]
        url_path: String,
    },
    V2 {
        #[serde(rename = "Sources")]
        sources: Vec<JuliaupVersionDBSource>,
    },
}

impl JuliaupVersionDBVersion {
    pub fn get_url_for_type(&self, preferred_type: &str) -> Option<&str> {
        match self {
            JuliaupVersionDBVersion::V1 { url_path } => Some(url_path.as_str()),
            JuliaupVersionDBVersion::V2 { sources } => sources
                .iter()
                .find(|s| s.source_type == preferred_type)
                .or_else(|| sources.first())
                .map(|s| s.url.as_str()),
        }
    }

    pub fn get_source_type_for_url(&self, url: &str) -> Option<&str> {
        match self {
            JuliaupVersionDBVersion::V1 { .. } => Some("tarball"),
            JuliaupVersionDBVersion::V2 { sources } => sources
                .iter()
                .find(|s| s.url == url)
                .map(|s| s.source_type.as_str()),
        }
    }

    pub fn get_tarball_url(&self) -> Option<&str> {
        match self {
            JuliaupVersionDBVersion::V1 { url_path } => Some(url_path.as_str()),
            JuliaupVersionDBVersion::V2 { sources } => sources
                .iter()
                .find(|s| s.source_type == "tarball")
                .map(|s| s.url.as_str()),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct JuliaupVersionDBChannel {
    #[serde(rename = "Version")]
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct JuliaupVersionDB {
    #[serde(rename = "AvailableVersions")]
    pub available_versions: HashMap<String, JuliaupVersionDBVersion>,
    #[serde(rename = "AvailableChannels")]
    pub available_channels: HashMap<String, JuliaupVersionDBChannel>,
    #[serde(rename = "Version")]
    pub version: String,
}
