use std::fs;

use anyhow::{Context,Result};

use crate::{config_file::{load_mut_config_db, save_config_db, JuliaupConfigApplication}, global_paths::GlobalPaths, operations::garbage_collect_versions};

pub fn run_command_app_remove(name: &str, paths: &GlobalPaths) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`app remove` command failed to load configuration data.")?;

    if !config_file.data.installed_apps.contains_key(name) {
        println!("Unknown app {}.", name);
    }
    else {
        let install_path = match &config_file.data.installed_apps.get(name).unwrap() {
            JuliaupConfigApplication::DirectDownloadApplication { path, url: _, local_etag: _, server_etag: _, julia_version: _, julia_depot: _, execution_aliases: _ } => {
                path
            }
        };

        fs::remove_dir_all(install_path).unwrap();

        config_file.data.installed_apps.remove(name).unwrap();

        garbage_collect_versions(&mut config_file.data, paths).unwrap();

        save_config_db(&mut config_file).unwrap();
    }

    return Ok(())
}
