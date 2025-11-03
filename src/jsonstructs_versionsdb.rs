use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// TODO: Future database schema v2 should support multiple ranked download sources.
// This would allow the database to specify preferred download formats per platform
// (e.g., DMG for macOS, with tarball fallback) instead of the current runtime
// string manipulation approach. The schema would look like:
// {
//   "Sources": [
//     {"Url": "path/to/file.dmg", "Type": "dmg", "Priority": 1},
//     {"Url": "path/to/file.tar.gz", "Type": "tarball", "Priority": 2}
//   ]
// }
// See discussion at: https://github.com/JuliaLang/juliaup/pull/1320
#[derive(Serialize, Deserialize)]
pub struct JuliaupVersionDBVersion {
    #[serde(rename = "UrlPath")]
    pub url_path: String,
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
