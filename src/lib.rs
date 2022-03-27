pub mod utils;
pub mod global_paths;
pub mod jsonstructs_versionsdb;
pub mod config_file;
pub mod versions_file;
pub mod operations;
pub mod command_add;
pub mod command_default;
pub mod command_gc;
pub mod command_link;
pub mod command_list;
pub mod command_status;
pub mod command_remove;
pub mod command_update;
pub mod command_config_symlinks;
pub mod command_config_backgroundselfupdate;
pub mod command_config_startupselfupdate;
pub mod command_config_modifypath;
pub mod command_initial_setup_from_launcher;
pub mod command_api;
pub mod command_selfupdate;
pub mod command_selfchannel;
pub mod command_selfuninstall;

include!(concat!(env!("OUT_DIR"), "/bundled_version.rs"));
include!(concat!(env!("OUT_DIR"), "/various_constants.rs"));
include!(concat!(env!("OUT_DIR"), "/built.rs"));

pub fn get_bundled_julia_full_version() -> &'static str {
    BUNDLED_JULIA_FULL_VERSION
}

pub fn get_juliaup_target() -> &'static str {
    JULIAUP_TARGET
}

pub fn get_own_version() -> anyhow::Result<semver::Version> {
    use semver::Version;
    use anyhow::Context;

    let version = Version::parse(PKG_VERSION)
        .with_context(|| "Failed to parse our own version.")?;

    Ok(version)
}
