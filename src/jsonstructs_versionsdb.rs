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

impl JuliaupVersionDB {
    /// The channel a new installation starts with. Platforms added ahead of
    /// their first Julia release (riscv64 before 1.14) only have nightlies.
    pub fn default_channel(&self) -> &'static str {
        if self.available_channels.contains_key("release") {
            "release"
        } else {
            "nightly"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(channels: &[&str]) -> JuliaupVersionDB {
        JuliaupVersionDB {
            available_versions: HashMap::new(),
            available_channels: channels
                .iter()
                .map(|c| {
                    let channel = JuliaupVersionDBChannel {
                        version: "1.14.0".to_string(),
                    };
                    (c.to_string(), channel)
                })
                .collect(),
            version: "1.0.0".to_string(),
        }
    }

    #[test]
    fn default_channel() {
        assert_eq!(db(&["release", "lts", "1.14"]).default_channel(), "release");
        assert_eq!(db(&[]).default_channel(), "nightly");
        assert_eq!(db(&["1.14", "alpha"]).default_channel(), "nightly");
    }
}
