use crate::config_file::*;
use anyhow::{bail, Context, Result, anyhow};
use crate::operations::{download_juliaup_version,download_extract_sans_parent};
use crate::get_juliaup_target;

pub fn run_command_selfupdate() -> Result<()> {
    let config_data =
        load_config_db().with_context(|| "`selfupdate` command failed to load configuration db.")?;

    let juliaup_channel = match config_data.juliaup_channel {
        Some(juliaup_channel) => juliaup_channel,
        None => "release".to_string()
    };

    let version_url = match juliaup_channel.as_str() {
        "release" => "https://julialang-s3.julialang.org/juliaup/RELEASECHANNELVERSION",
        "releasepreview" => "https://julialang-s3.julialang.org/juliaup/RELEASEPREVIEWCHANNELVERSION",
        "dev" => "https://julialang-s3.julialang.org/juliaup/DEVCHANNELVERSION",
        _ => bail!("Juliaup is configured to a channel named '{}' that does not exist.", &juliaup_channel)
    };

    let version = download_juliaup_version(version_url)?;

    let juliaup_target = get_juliaup_target();

    let new_juliaup_url = format!("https://github.com/JuliaLang/juliaup/releases/download/v{}/{}.tar.gz", version, juliaup_target);

    let my_own_path = std::env::current_exe()
        .with_context(|| "Could not determine the path of the running exe.")?;

    let my_own_folder = my_own_path.parent()
        .ok_or_else(|| anyhow!("Could not determine parent."))?;

    println!("We are on the juliaup channel '{}'.", juliaup_channel);
    println!("The version URL is {}.", version_url);
    println!("The current version is {}.", version);
    println!("We will replace {:?}.", my_own_path);
    println!("We will replace files in {:?}.", my_own_folder);
    println!("We are on {}.", juliaup_target);
    println!("We will download from {}.", new_juliaup_url);

    download_extract_sans_parent(&new_juliaup_url, &my_own_folder, 2)?;

    Ok(())
}
