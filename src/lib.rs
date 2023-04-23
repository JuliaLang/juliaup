use anyhow::Context;

pub mod command_add;
pub mod command_api;
pub mod command_config_backgroundselfupdate;
pub mod command_config_modifypath;
pub mod command_config_startupselfupdate;
pub mod command_config_symlinks;
pub mod command_config_versionsdbupdate;
pub mod command_default;
pub mod command_gc;
pub mod command_info;
pub mod command_initial_setup_from_launcher;
pub mod command_link;
pub mod command_list;
pub mod command_remove;
pub mod command_selfchannel;
pub mod command_selfuninstall;
pub mod command_selfupdate;
pub mod command_status;
pub mod command_update;
pub mod command_update_version_db;
pub mod config_file;
pub mod global_paths;
pub mod jsonstructs_versionsdb;
pub mod operations;
pub mod utils;
pub mod versions_file;

include!(concat!(env!("OUT_DIR"), "/bundled_version.rs"));
include!(concat!(env!("OUT_DIR"), "/various_constants.rs"));
include!(concat!(env!("OUT_DIR"), "/built.rs"));

pub fn get_bundled_julia_version() -> &'static str {
    BUNDLED_JULIA_VERSION
}

pub fn get_bundled_dbversion() -> anyhow::Result<semver::Version> {
    let dbversion = semver::Version::parse(BUNDLED_DB_VERSION)
        .with_context(|| "Failed to parse our own db version.")?;

    Ok(dbversion)
}

pub fn get_juliaup_target() -> &'static str {
    JULIAUP_TARGET
}

pub fn get_own_version() -> anyhow::Result<semver::Version> {
    use semver::Version;

    let version =
        Version::parse(PKG_VERSION).with_context(|| "Failed to parse our own version.")?;

    Ok(version)
}
