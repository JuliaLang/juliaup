use crate::config_file::JuliaupConfig;
use crate::config_file::JuliaupConfigChannel;
use crate::config_file::JuliaupConfigVersion;
use crate::get_bundled_julia_full_version;
use crate::jsonstructs_versionsdb::JuliaupVersionDB;
use crate::utils::get_arch;
use crate::utils::{get_juliaserver_base_url, get_juliaserver_nightly_base_url};
use crate::utils::get_juliaup_home_path;
use crate::utils::parse_versionstring;
use crate::utils::get_bin_dir;
use anyhow::{anyhow, Context, Result};
use console::style;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    io::Read,
    path::{Component::Normal, Path, PathBuf},
};
use tar::Archive;
use semver::Version;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::PermissionsExt;

fn unpack_sans_parent<R, P>(mut archive: Archive<R>, dst: P, levels_to_skip: usize) -> Result<()>
where
    R: Read,
    P: AsRef<Path>,
{
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path: PathBuf = entry
            .path()?
            .components()
            .skip(levels_to_skip) // strip top-level directory
            .filter(|c| matches!(c, Normal(_))) // prevent traversal attacks TODO We should actually abort if we come across a non-standard path element
            .collect();
        entry.unpack(dst.as_ref().join(path))?;
    }
    Ok(())
}

pub fn download_extract_sans_parent(url: &String, target_path: &Path, levels_to_skip: usize) -> Result<()> {
    let response = ureq::get(url)
        .call()
        .with_context(|| format!("Failed to download from url `{}`.", url))?;

    let content_length = response.header("Content-Length").and_then(|v| v.parse::<u64>().ok() );

    let pb = match content_length {
        Some(content_length) => ProgressBar::new(content_length),
        None => ProgressBar::new_spinner(),
    };
    
    pb.set_prefix("  Downloading:");
    pb.set_style(ProgressStyle::default_bar()
    .template("{prefix:.cyan.bold} [{bar}] {bytes}/{total_bytes} eta: {eta}")
                .progress_chars("=> "));

    let foo = pb.wrap_read(response.into_reader());

    let tar = GzDecoder::new(foo);
    let archive = Archive::new(tar);
    unpack_sans_parent(archive, &target_path, levels_to_skip)
        .with_context(|| format!("Failed to extract downloaded file from url `{}`.", url))?;
    Ok(())
}

pub fn download_juliaup_version(url: &str) -> Result<Version> {
    let response = ureq::get(url)
        .call()?
        .into_string()
        .with_context(|| format!("Failed to download from url `{}`.", url))?
        .trim()
        .to_string();

    let version = Version::parse(&response)
        .with_context(|| format!("`download_juliaup_version` failed to parse `{}` as a valid semversion.", response))?;

    Ok(version)
}

pub fn install_version(
    fullversion: &String,
    config_data: &mut JuliaupConfig,
    version_db: &JuliaupVersionDB,
) -> Result<()> {
    let nightly = version_db.available_versions
        .get(fullversion)
        .ok_or(anyhow!("Version {} does not exist", fullversion))?
        .nightly;

    // Return immediately if the version is already installed.
    if !nightly && config_data.installed_versions.contains_key(fullversion) {
        return Ok(());
    }

    // TODO At some point we could put this behind a conditional compile, we know
    // that we don't ship a bundled version for some platforms.
    let platform = get_arch()?;
    let full_version_string_of_bundled_version = format!("{}~{}", get_bundled_julia_full_version(), platform);
    let my_own_path = std::env::current_exe()?;
    let path_of_bundled_version = my_own_path
        .parent()
        .unwrap() // unwrap OK because we can't get a path that does not have a parent
        .join("BundledJulia");

    let child_target_foldername = format!("julia-{}", fullversion);
    let target_path = get_juliaup_home_path()
        .with_context(|| "Failed to retrieve juliaup folder while trying to install new version.")?
        .join(&child_target_foldername);
    std::fs::create_dir_all(target_path.parent().unwrap())?;

    if fullversion == &full_version_string_of_bundled_version && path_of_bundled_version.exists() {
        let mut options = fs_extra::dir::CopyOptions::new();
        options.overwrite = true;
        options.content_only = true;
        fs_extra::dir::copy(path_of_bundled_version, target_path, &options)?;        
    } else {
        let juliaupserver_base = if nightly {
            get_juliaserver_nightly_base_url()
        } else {
            get_juliaserver_base_url()
        }.with_context(|| "Failed to get Juliaup server base URL.")?;

        let download_url_path = &version_db
            .available_versions
            .get(fullversion)
            .ok_or(anyhow!(
                "Failed to find download url in versions db for '{}'.",
                fullversion
            ))?
            .url_path;

        let download_url = juliaupserver_base.join(download_url_path)
            .with_context(|| format!("Failed to construct a valid url from '{}' and '{}'.", juliaupserver_base, download_url_path))?;
        
        let (platform, version) = parse_versionstring(fullversion).with_context(|| format!(""))?;

        eprintln!("{} Julia {} ({}).", style("Installing").green().bold(), version, platform);

        download_extract_sans_parent(&download_url.to_string(), &target_path, 1)?;
    }

    let mut rel_path = PathBuf::new();
    rel_path.push(".");
    rel_path.push(&child_target_foldername);

    config_data.installed_versions.insert(
        fullversion.clone(),
        JuliaupConfigVersion {
            path: rel_path.to_string_lossy().into_owned(),
        },
    );

    Ok(())
}

pub fn garbage_collect_versions(config_data: &mut JuliaupConfig) -> Result<()> {
    let home_path = get_juliaup_home_path().with_context(|| {
        "Failed to retrieve juliaup folder while trying to garbage collect versions."
    })?;

    let mut versions_to_uninstall: Vec<String> = Vec::new();
    for (installed_version, detail) in &config_data.installed_versions {
        if config_data.installed_channels.iter().all(|j| match &j.1 {
            JuliaupConfigChannel::SystemChannel { version } => version != installed_version,
            JuliaupConfigChannel::LinkedChannel {
                command: _,
                args: _,
            } => true,
        }) {
            let path_to_delete = home_path.join(&detail.path);
            let display = path_to_delete.display();

            match std::fs::remove_dir_all(&path_to_delete) {
                Err(_) => eprintln!("WARNING: Failed to delete {}. You can try to delete at a later point by running `juliaup gc`.", display),
                Ok(_) => ()
            };
            versions_to_uninstall.push(installed_version.clone());
        }
    }

    for i in versions_to_uninstall {
        config_data.installed_versions.remove(&i);
    }

    Ok(())
}

fn _remove_symlink(
    symlink_path: &Path,
) -> Result<()> {
    std::fs::create_dir_all(symlink_path.parent().unwrap())?;

    if symlink_path.exists() {
        std::fs::remove_file(&symlink_path)?;
    }

    Ok(())
}

pub fn remove_symlink(
    symlink_name: &String,
) -> Result<()> {
    let symlink_path = get_bin_dir()
        .with_context(|| "Failed to retrieve binary directory while trying to remove a symlink.")?
        .join(&symlink_name);

    eprintln!("{} {}.", style("Deleting symlink").cyan().bold(), symlink_name);

    _remove_symlink(&symlink_path)?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn create_symlink(
    channel: &JuliaupConfigChannel,
    symlink_name: &String,
) -> Result<()> {

    let symlink_path = get_bin_dir()
        .with_context(|| "Failed to retrieve binary directory while trying to create a symlink.")?
        .join(&symlink_name);

    _remove_symlink(&symlink_path)?;

    match channel {
        JuliaupConfigChannel::SystemChannel { version } => {
            let child_target_fullname = format!("julia-{}", version);

            let target_path = get_juliaup_home_path()
                .with_context(|| "Failed to retrieve juliaup folder while trying to create a symlink.")?
                .join(&child_target_fullname);

            let (platform, version) = parse_versionstring(version).with_context(|| format!(""))?;

            eprintln!("{} {} for Julia {} ({}).", style("Creating symlink").cyan().bold(), symlink_name, version, platform);

            std::os::unix::fs::symlink(target_path.join("bin").join("julia"), &symlink_path)
                .with_context(|| format!("failed to create symlink `{}`.", symlink_path.to_string_lossy()))?;
        },
        JuliaupConfigChannel::LinkedChannel { command, args } => {
            let formatted_command = match args {
                Some(x) => format!("{} {}", command, x.join(" ")),
                None    => command.clone(),
            };

            eprintln!("{} {} for `{}`", style("Creating shim").cyan().bold(), symlink_name, formatted_command);

            std::fs::write(
                &symlink_path,
                format!(
r#"#!/bin/sh
{} "$@"
"#,
                    formatted_command,
                ),
            ).with_context(|| format!("failed to create shim `{}`.", symlink_path.to_string_lossy()))?;

            // set as executable
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&symlink_path, perms)
                .with_context(|| format!("failed to change permissions for shim `{}`.", symlink_path.to_string_lossy()))?;
        },
    };

    if let Ok(path) = std::env::var("PATH") {
        if !path.split(":").any(|p| Path::new(p) == symlink_path) {
            eprintln!(
                "Symlink {} added in {}. Add this directory to the system PATH to make the command available in your shell.",
                &symlink_name, symlink_path.display(),
            );
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn create_symlink(_: &JuliaupConfigChannel, _: &String) -> Result<()> { Ok(()) }
