use crate::config_file::JuliaupConfig;
use crate::config_file::JuliaupConfigVersion;
use crate::utils::get_juliaup_home_path;
use crate::utils::parse_versionstring;
use crate::versions_file::JuliaupVersionDB;
use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::path::Path;
use tar::Archive;
use tempfile::Builder;

fn download_extract(url: &String, target_path: &Path) -> Result<()> {
    let response =ureq::get(url)
        .call()
        .with_context(|| format!("Failed to download from url `{}`.", url))?;

    let tar = GzDecoder::new(response.into_reader());
    let mut archive = Archive::new(tar);
    
    archive.unpack(&target_path)
        .with_context(|| format!("Failed to extract downloaded file from url `{}`.", url))?;
    
    Ok(())
}

pub fn install_version(
    fullversion: &String,
    config_data: &mut JuliaupConfig,
    version_db: &JuliaupVersionDB,
) -> Result<()> {
    let download_url = version_db
        .available_versions
        .get(fullversion)
        .ok_or(anyhow!(
            "Failed to find download url in versions db for '{}'.",
            fullversion
        ))?
        .url
        .clone();

    let (platform, version) = parse_versionstring(fullversion).with_context(|| format!(""))?;

    let child_target_foldername = format!("julia-{}", fullversion);

    let target_path = get_juliaup_home_path()
        .with_context(|| "Failed to retrieve juliap folder while trying to install new version.")?
        .join(&child_target_foldername);

    println!("Installing Julia {} ({}).", version, platform);

    let tmp_dir = Builder::new().prefix("juliaup").tempdir()?;

    let tmp_dir_path = tmp_dir.path();

    download_extract(&download_url, tmp_dir_path)?;

    let child_folders = fs::read_dir(tmp_dir_path)?.collect::<Result<Vec<_>, io::Error>>()?;

    if child_folders.len() != 1 {
        return Err(anyhow!(
            "The archive for this version has a folder structure that juliaup does not understand."
        ));
    }

    fs::rename(child_folders[0].path(), target_path)?;

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
    let default_version = &config_data.default;

    let home_path = get_juliaup_home_path().with_context(|| {
        "Failed to retrieve juliap folder while trying to garbage collect versions."
    })?;

    let mut versions_to_uninstall: Vec<String> = Vec::new();
    for (version, detail) in &config_data.installed_versions {
        if default_version != version
            && config_data.installed_channels.iter().all(|j| {
                // if haskey(j[2], "Version")
                // 	return j[2]["Version"] != version
                // else
                // 	return true
                // end
                &j.1.version != version
            })
        {
            let path_to_delete = home_path.join(&detail.path);
            let display = path_to_delete.display();

            match std::fs::remove_dir_all(&path_to_delete) {
            Err(_) => println!("WARNING: Failed to delete {}. You can try to delete at a later point by running `juliaup gc`.", display),
            Ok(_) => ()
        };
            versions_to_uninstall.push(version.clone());
        }
    }

    for i in versions_to_uninstall {
        config_data.installed_versions.remove(&i);
    }

    Ok(())
}


