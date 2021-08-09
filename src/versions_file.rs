use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use crate::utils::get_juliaup_home_path;
use anyhow::{Context,Result};

#[derive(Serialize, Deserialize)]
pub struct JuliaupVersionDBVersion {
    #[serde(rename = "Url")]
    pub url: String
}

#[derive(Serialize, Deserialize)]
pub struct JuliaupVersionDBChannel {
    #[serde(rename = "Version")]
    pub version: String
}

#[derive(Serialize, Deserialize)]
pub struct JuliaupVersionDB {
    #[serde(rename = "AvailableVersions")]
    pub available_versions: HashMap<String,JuliaupVersionDBVersion>,
    #[serde(rename = "AvailableChannels")]
    pub available_channels: HashMap<String,JuliaupVersionDBChannel>
}

pub fn load_versions_db() -> Result<JuliaupVersionDB> {    
    let vendored_db = include_str!(concat!(env!("OUT_DIR"), "/versionsdb.json"));

    let version_db_search_paths = [		
        // TODO Reenable second path
        // joinpath(Sys.BINDIR, "..", "..", "VersionsDB", "juliaup-versionsdb-winnt-$(Int===Int64 ? "x64" : "x86").json"),
        get_juliaup_home_path()
            .with_context(|| "Failed to determine versions db file path.")?
            .join("juliaup-versionsdb-winnt-x64.jsonIGNORE"),
    ];

    for version_db_path in version_db_search_paths {
        let display = version_db_path.display();

        let file = match File::open(&version_db_path) {
            // If we can't open this file, just try the next one.
            Err(_) => continue,
            Ok(file) => file,
        };
    
        let reader = BufReader::new(file);
    
        let db: JuliaupVersionDB = serde_json::from_reader(reader)
            .with_context(|| format!("Failed to parse version db at '{}'.", display))?;

        return Ok(db);
    }

    let db: JuliaupVersionDB = serde_json::from_str(&vendored_db)
        .with_context(|| "Failed to parse vendored version db.")?;

    Ok(db)
}
