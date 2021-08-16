use std::fs::File;
use std::io::BufReader;
use crate::utils::get_juliaup_home_path;
use anyhow::{Context,Result};
use crate::jsonstructs_versionsdb::JuliaupVersionDB;

pub fn load_versions_db() -> Result<JuliaupVersionDB> {    
    let vendored_db = include_str!(concat!(env!("OUT_DIR"), "/versionsdb.json"));

    // TODO Reenable once we have an automatic way to update this file
    // let version_db_path =
    //     get_juliaup_home_path()
    //         .with_context(|| "Failed to determine versions db file path.")?
    //         .join("juliaup-versionsdb-winnt-x64.json");


    // let file = File::open(&version_db_path);
    
    // if let Ok(file) = file {
    //     let reader = BufReader::new(file);
    
    //     let db: JuliaupVersionDB = serde_json::from_reader(reader)
    //         .with_context(|| format!("Failed to parse version db at '{}'.", version_db_path.display()))?;

    //     return Ok(db);
    // }

    let db: JuliaupVersionDB = serde_json::from_str(&vendored_db)
        .with_context(|| "Failed to parse vendored version db.")?;

    Ok(db)
}
