use std::fs;
use std::str::FromStr;

use crate::config_file::{load_mut_config_db, save_config_db, JuliaupConfigApplication, JuliaupConfigExcutionAlias};
use crate::global_paths::GlobalPaths;
use crate::operations::{download_file, install_version};
use crate::versions_file::load_versions_db;
use anyhow::{Context, Result};
use tempfile::Builder;
use normpath::PathExt;

pub fn run_command_app_add(url: &str, paths: &GlobalPaths) -> Result<()> {
    let download_uri_project = format!("{}/Project.toml", url);
    let download_uri_manifest = format!("{}/Manifest.toml", url);

    let temp_dir = Builder::new()
        .prefix("julia-temp-app-")
        .tempdir_in(&paths.juliauphome)
        .expect("Failed to create temporary directory");

    download_file(&download_uri_project, temp_dir.path(), "Project.toml").unwrap();
    download_file(&download_uri_manifest, temp_dir.path(), "Manifest.toml").unwrap();

    let project_content = fs::read_to_string(temp_dir.path().join("Project.toml")).unwrap();
    let project_parsed = toml_edit::DocumentMut::from_str(&project_content).unwrap();

    let manifest_content = fs::read_to_string(temp_dir.path().join("Manifest.toml")).unwrap();
    let manifest_parsed = toml_edit::DocumentMut::from_str(&manifest_content).unwrap();

    let app_name = project_parsed.as_table().get_key_value("name").unwrap().1.as_str().unwrap();
    let julia_version = manifest_parsed.as_table().get_key_value("julia_version").unwrap().1.as_str().unwrap();

    let target_path = paths.juliauphome.join("applications").join(app_name);

    let exec_aliases: Vec<(String, String)> = project_parsed
        .as_table()
        .get_key_value("executionaliases")
        .unwrap()
        .1
        .clone()
        .into_table()
        .unwrap()
        .iter()
        .map(|i| (i.0.to_string(), i.1.clone().into_value().unwrap().as_str().unwrap().to_string()))
        .collect();

    if target_path.exists() {
        std::fs::remove_dir_all(&target_path)?;
    }
    std::fs::create_dir_all(paths.juliauphome.join("applications")).unwrap();
    std::fs::rename(temp_dir.into_path(), &target_path)?;

    let version_db =
        load_versions_db(paths).with_context(|| "`add app` command failed to load versions db.")?;

    let asdf = version_db.available_channels.get(julia_version).unwrap();

    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`app add` command failed to load configuration data.")?;

    install_version(&asdf.version, &mut config_file.data, &version_db, paths).unwrap();

    config_file.data.installed_apps.insert(
        app_name.to_string(),
        JuliaupConfigApplication::DirectDownloadApplication { 
            path: target_path.to_str().unwrap().to_string(),
            url: url.to_string(),
            local_etag: "".to_string(),
            server_etag: "".to_string(),
            version: asdf.version.to_string(),
            execution_aliases: exec_aliases.iter().map(|i| (i.0.clone(), JuliaupConfigExcutionAlias { target: i.1.to_string() })).collect()
        }
    );

    save_config_db(&mut config_file).unwrap();

    let absolute_path = &paths.juliaupconfig
        .parent()
        .unwrap() // unwrap OK because there should always be a parent
        .join(config_file.data.installed_versions.get(&asdf.version).unwrap().path.clone())
        .join("bin")
        .join(format!("julia{}", std::env::consts::EXE_SUFFIX))
        .normalize().unwrap();

    std::process::Command::new(absolute_path)
        .env("JULIA_PROJECT", target_path)
        .arg("-e")
        .arg("using Pkg; Pkg.instantiate()")
        .status()
        .unwrap();

    return Ok(())
}
