use std::{fs::File, io::BufReader};

// use std::fs::File;
// use std::io::BufReader;
// use crate::utils::get_juliaup_home_path;
use crate::{
    get_bundled_dbversion, global_paths::GlobalPaths, jsonstructs_versionsdb::JuliaupVersionDB,
};
use anyhow::{Context, Result};
use semver::Version;

fn load_vendored_db() -> Result<JuliaupVersionDB> {
    let vendored_db = include_str!(concat!(env!("OUT_DIR"), "/versionsdb.json"));

    let db: JuliaupVersionDB = serde_json::from_str(vendored_db)
        .with_context(|| "Failed to parse vendored version db.")?;

    Ok(db)
}

pub fn load_versions_db(paths: &GlobalPaths) -> Result<JuliaupVersionDB> {
    let file = File::open(&paths.versiondb);

    let local_version_db = match file {
        Ok(file) => {
            let reader = BufReader::new(&file);

            serde_json::from_reader::<BufReader<&std::fs::File>, JuliaupVersionDB>(reader).ok()
        }
        Err(_) => None,
    };

    let db = match local_version_db {
        Some(local_version_db) => {
            if let Ok(version) = Version::parse(&local_version_db.version) {
                if version >= get_bundled_dbversion().unwrap() {
                    local_version_db
                } else {
                    load_vendored_db().with_context(|| "Failed to load vendored version db.")?
                }
            } else {
                load_vendored_db().with_context(|| "Failed to load vendored version db.")?
            }
        }
        None => load_vendored_db().with_context(|| "Failed to load vendored version db.")?,
    };

    Ok(db)
}
