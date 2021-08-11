use anyhow::{Context, Result};
use juliaup::config_file::{
    load_config_db, JuliaupConfig, JuliaupConfigChannel
};
use juliaup::utils::get_juliaupconfig_path;
use juliaup::versions_file::load_versions_db;
use juliaup::jsonstructs_versionsdb::JuliaupVersionDB;
use normpath::PathExt;
use std::path::Path;
use std::path::PathBuf;

fn do_initial_setup(juliaupconfig_path: &Path) -> Result<()> {
    

    if !juliaupconfig_path.exists() {
        let my_own_path = std::env::current_exe()?;

        std::process::Command::new(my_own_path.parent().unwrap().join(format!("juliaup{}", std::env::consts::EXE_SUFFIX)))
            .arg("46029ef5-0b73-4a71-bff3-d0d05de42aac") // This is our internal command to do the initial setup
            .status()?;
    
    //     }
    }
    Ok(())
}

fn check_channel_uptodate(
    channel: &str,
    current_version: &str,
    versions_db: &JuliaupVersionDB,
) -> Result<()> {
    let latest_version = &versions_db.available_channels.get(channel).unwrap().version;
    // return Err(anyhow!("The configured channel `{}` does not exist in the versions database.", channel));

    if latest_version != current_version {
        println!("The latest version of Julia in the `{}` channel is {}. You currently have `{}` installed. Run:", channel, latest_version, current_version);
        println!();
        println!("  juliaup update");
        println!();
        println!(
            "to install Julia {} and update the `{}` channel to that version.",
            latest_version, channel
        );
    }
    Ok(())
}

fn get_julia_path_from_channel(
    versions_db: &JuliaupVersionDB,
    config_data: &JuliaupConfig,
    channel: &str,
    juliaupconfig_path: &Path,
) -> Result<(PathBuf, Vec<String>)> {
    let channel_info = config_data.installed_channels.get(channel).unwrap(); // TODO Proper error throwing here.
    // {
    // 	if channel_is_from_commandline {
    //         return Err(anyhow!("No channel with name `{}` exists in the juliaup configuration file.", channel));
    // 	} else {
    //         return Err(anyhow!("No channel named `{}` exists. Please use the name of an installed channel.", channel));
    // 	}
    // }

    match channel_info {
        JuliaupConfigChannel::LinkedChannel { command, args } => {
            return Ok((PathBuf::from(command), args.clone().unwrap()))
        }
        JuliaupConfigChannel::SystemChannel { version } => {
            let path = &config_data.installed_versions.get(version).unwrap().path;
            //     throw JuliaupConfigError("The channel `" + channel + "` points to a Julia version that is not installed.");
            check_channel_uptodate(channel, version, versions_db)?;

            let absolute_path = juliaupconfig_path
                .parent()
                .unwrap()
                .join(path)
                .join("bin")
                .join(format!("julia{}", std::env::consts::EXE_SUFFIX))
                .normalize()?;
            // return normalizedPath;
            return Ok((absolute_path.into_path_buf(), Vec::new()));
        }
    }
}

fn main() -> Result<()> {
    // TODO SetConsoleTitle(L"Julia");

    let juliaupconfig_path = get_juliaupconfig_path()?;

    do_initial_setup(&juliaupconfig_path)?;

    let config_data =
        load_config_db().with_context(|| "`status` command failed to load configuration file.")?;

    let versiondb_data =
        load_versions_db().with_context(|| "`status` command failed to load versions db.")?;

    let mut julia_channel_to_use = config_data.default.clone();

    let args: Vec<String> = std::env::args().collect();

    // let mut julia_version_from_cmd_line = false;

    if args.len() > 1 {
        let first_arg = &args[1];

        if first_arg.starts_with("+") {
            julia_channel_to_use = first_arg[1..].to_string();
            // julia_version_from_cmd_line = true;
        }
    }

    let (julia_path, julia_args) = get_julia_path_from_channel(
        &versiondb_data,
        &config_data,
        &julia_channel_to_use,
        &juliaupconfig_path,
    )?;

    let mut new_args: Vec<String> = Vec::new();

    for i in julia_args {
        new_args.push(i);
    }

    for (i, v) in args.iter().skip(1).enumerate() {
        if i > 1 || !v.starts_with("+") {
            new_args.push(v.clone());
        }
    }

    std::process::Command::new(julia_path)
        .args(&new_args)
        .status()?;

    Ok(())
}
