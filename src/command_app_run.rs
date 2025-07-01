use std::collections::HashMap;

use crate::{config_file::{load_config_db, JuliaupConfigApplication}, global_paths::GlobalPaths};
use anyhow::{Context,Result};
use normpath::PathExt;

pub fn run_command_app_run(name: &str, args: &Vec<String>, paths: &GlobalPaths) -> Result<()> {

    let config_file = load_config_db(paths)
        .with_context(|| "`app run` command failed to load configuration data.")?;

    let target: HashMap<String,(String,String,String,String)> = config_file
        .data
        .installed_apps
        .iter()
        .flat_map(|i| match&i.1 {
            JuliaupConfigApplication::DevedApplication { path, julia_version, julia_depot, execution_aliases } => execution_aliases.iter().map(|j| (j.0.clone(), (j.1.target.clone(), path.clone(), julia_version.clone(), julia_depot.clone())))
        })
        .map(|i| (i.0.clone(), (i.1.0.clone(), i.1.1.clone(), i.1.2.clone(), i.1.3.clone())))
        .collect();

    if target.contains_key(name) {
        let foo = target.get(name).unwrap();

        let parts: Vec<&str> = foo.0.split(".").collect();

        // println!("First arg {}, second arg {}", foo.0, foo.1)

        let target_path = foo.1.clone();

        let julia_binary_path = &paths.juliaupconfig
            .parent()
            .unwrap() // unwrap OK because there should always be a parent
            .join(config_file.data.installed_versions.get(&foo.2).unwrap().path.clone())
            .join("bin")
            .join(format!("julia{}", std::env::consts::EXE_SUFFIX))
            .normalize().unwrap();

        std::process::Command::new(julia_binary_path)
            .arg(format!("--project={}", target_path))
            // .env("JULIA_PROJECT", target_path)
            .env("JULIA_DEPOT_PATH", foo.3.clone())
            .arg("-e")
            .arg(format!("import {}; {}(ARGS)", parts[0], foo.0))
            .args(args)
            .status()
            .unwrap();

    }
    else {
        println!("Could not find app.");
    }
    return Ok(())    
}
