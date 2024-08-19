use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use crate::config_file::{load_mut_config_db, save_config_db, JuliaupConfigApplication, JuliaupConfigExcutionAlias};
use crate::global_paths::GlobalPaths;
use crate::operations::install_version;
use crate::versions_file::load_versions_db;
use anyhow::{Context, Result};
use bstr::ByteVec;
use normpath::PathExt;

pub fn run_command_app_add(path: &str, paths: &GlobalPaths) -> Result<()> {
    let app_folder_path = PathBuf::from(path);

    let project_path = app_folder_path.join("Project.toml");
    let manifest_path = app_folder_path.join("Manifest.toml");

    let project_content = fs::read_to_string(project_path).unwrap();
    let project_parsed = toml_edit::DocumentMut::from_str(&project_content).unwrap();

    let manifest_content = fs::read_to_string(&manifest_path).unwrap();
    let manifest_parsed = toml_edit::DocumentMut::from_str(&manifest_content).unwrap();

    let app_name = project_parsed.as_table().get_key_value("name").unwrap().1.as_str().unwrap();
    let julia_version = manifest_parsed.as_table().get_key_value("julia_version").unwrap().1.as_str().unwrap();

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

    let version_db =
        load_versions_db(paths).with_context(|| "`add app` command failed to load versions db.")?;

    let asdf = version_db.available_channels.get(julia_version).unwrap();

    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`app add` command failed to load configuration data.")?;

    install_version(&asdf.version, &mut config_file.data, &version_db, paths).unwrap();

    let julia_binary_path = &paths.juliaupconfig
        .parent()
        .unwrap() // unwrap OK because there should always be a parent
        .join(config_file.data.installed_versions.get(&asdf.version).unwrap().path.clone())
        .join("bin")
        .join(format!("julia{}", std::env::consts::EXE_SUFFIX))
        .normalize().unwrap();

    let depot_detection_output = std::process::Command::new(julia_binary_path)
        .arg("-e")
        .arg("println(Base.DEPOT_PATH[1])")
        .output()
        .unwrap();

    let depot_detection_output = depot_detection_output.stdout.into_string().unwrap().trim().to_string();

    config_file.data.installed_apps.insert(
        app_name.to_string(),
        JuliaupConfigApplication::DevedApplication { 
            path: app_folder_path.to_str().unwrap().to_string(),
            julia_version: asdf.version.to_string(),
            julia_depot: depot_detection_output,
            execution_aliases: exec_aliases.iter().map(|i| (i.0.clone(), JuliaupConfigExcutionAlias { target: i.1.to_string() })).collect()
        }
    );

    save_config_db(&mut config_file).unwrap();
    
    std::process::Command::new(julia_binary_path)
        .env("JULIA_PROJECT", &app_folder_path)
        .arg("-e")
        .arg("using Pkg; Pkg.instantiate()")
        .status()
        .unwrap();

    // #[cfg(feature = "winpkgidentityext")]
    {
        use windows::Management::Deployment::{RegisterPackageOptions, PackageManager};

        let package_manager = PackageManager::new().unwrap();
        let register_package_options = RegisterPackageOptions::new().unwrap();
        register_package_options.SetAllowUnsigned(true)?;

        let self_location = std::env::current_exe().unwrap();
        let self_location = self_location.parent().unwrap().parent().unwrap().parent().unwrap().join("stringbuislders.xml");

        println!("WE ARE AT {:?}", self_location);

        let external_loc =
            windows::Foundation::Uri::CreateUri(&windows::core::HSTRING::from("C:\\Users\\david\\source\\juliaup\\AppxManifest.xml"))
                .unwrap();

        let asdf = package_manager.RegisterPackageByUriAsync(&external_loc, &register_package_options).unwrap().get().unwrap();

        println!("DEPLOY WAS {:?} with {:?}", asdf.IsRegistered().unwrap(), asdf.ErrorText().unwrap());
    }

    return Ok(())
}
