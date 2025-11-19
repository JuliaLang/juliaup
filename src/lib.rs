use anyhow::Context;

pub mod command;

pub mod cli;
pub mod cli_styles;
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
