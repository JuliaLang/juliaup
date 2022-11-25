use std::io::BufReader;

use crate::{global_paths::GlobalPaths, get_bundled_dbversion, operations::download_versiondb, get_juliaup_target, config_file::save_config_db};
use anyhow::{Result, bail, Context};
use crate::jsonstructs_versionsdb::JuliaupVersionDB;
use crate::operations::download_juliaup_version;
use crate::config_file::load_mut_config_db;
use crate::utils::get_juliaserver_base_url;

pub fn run_command_update_version_db(paths: &GlobalPaths) -> Result<()> {
    let mut config_file =
        load_mut_config_db(paths).with_context(|| "`run_command_update_version_db` command failed to load configuration db.")?;

    #[cfg(feature = "selfupdate")]
    let juliaup_channel = match &config_file.self_data.juliaup_channel {
        Some(juliaup_channel) => juliaup_channel.to_string(),
        None => "release".to_string()
    };

    // TODO Figure out how we can learn about the correctn Juliaup channel here
    #[cfg(not(feature = "selfupdate"))]
    let juliaup_channel = "release".to_string();

    let juliaupserver_base = get_juliaserver_base_url()
            .with_context(|| "Failed to get Juliaup server base URL.")?;
            
    let dbversion_url_path = match juliaup_channel.as_str() {
        "release" => "juliaup/RELEASECHANNELDBVERSION",
        "releasepreview" => "juliaup/RELEASEPREVIEWCHANNELDBVERSION",
        "dev" => "juliaup/DEVCHANNELDBVERSION",
        _ => bail!("Juliaup is configured to a channel named '{}' that does not exist.", &juliaup_channel)
    };

    let dbversion_url = juliaupserver_base.join(dbversion_url_path)
        .with_context(|| format!("Failed to construct a valid url from '{}' and '{}'.", juliaupserver_base, dbversion_url_path))?;

    let online_dbversion = download_juliaup_version(&dbversion_url.to_string())
        .with_context(|| "Failed to download current version db version.")?;

    config_file.data.last_version_db_update = Some(chrono::Utc::now());

    save_config_db(&mut config_file)
        .with_context(|| "Failed to save configuration file.")?;

    let bundled_dbversion = get_bundled_dbversion()
        .with_context(|| "Failed to determine the bundled version db version.")?;

    let local_dbversion = match std::fs::OpenOptions::new().read(true).open(&paths.versiondb) {
        Ok(file) => {
            let reader = BufReader::new(&file);

            if let Ok(versiondb) = serde_json::from_reader::<BufReader<&std::fs::File>, JuliaupVersionDB>(reader) {
                if let Ok(version) = semver::Version::parse(&versiondb.version) {
                    Some(version)
                } else {
                    None
                }
            } else {
                None
            }                
        }
        Err(_) => { 
            None 
        }
    };

    eprintln!("Bundled version db: {}", bundled_dbversion);
    eprintln!("Online version db: {}", online_dbversion);
    eprintln!("Local cached version db: {:?}", local_dbversion);

    if online_dbversion>bundled_dbversion {      
        if local_dbversion.is_none() || online_dbversion>local_dbversion.unwrap() {
            let onlineversiondburl = juliaupserver_base.join(&format!("juliaup/versiondb/versiondb-{}-{}.json", online_dbversion, get_juliaup_target()))
                .with_context(|| "Failed to construct URL for version db download.")?;

            download_versiondb(&onlineversiondburl.to_string(), &paths.versiondb)
                .with_context(|| format!("Failed to download new version db from {}.", onlineversiondburl))?;            
        }
    }
    else if local_dbversion.is_some() {
        // If the bundled version is up-to-date we can delete any cached version db json file
        let _ = std::fs::remove_file(&paths.versiondb);
    }
    
    Ok(())
}
