use crate::config_file::*;
use crate::utils::get_juliaserver_base_url;
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

    let juliaupserver_base = get_juliaserver_base_url()
            .with_context(|| "Failed to get Juliaup server base URL.")?;
            
    let version_url_path = match juliaup_channel.as_str() {
        "release" => "juliaup/RELEASECHANNELVERSION",
        "releasepreview" => "juliaup/RELEASEPREVIEWCHANNELVERSION",
        "dev" => "juliaup/DEVCHANNELVERSION",
        _ => bail!("Juliaup is configured to a channel named '{}' that does not exist.", &juliaup_channel)
    };

    let version_url = juliaupserver_base.join(version_url_path)
        .with_context(|| format!("Failed to construct a valid url from '{}' and '{}'.", juliaupserver_base, version_url_path))?;

    let version = download_juliaup_version(&version_url.to_string())?;

    let juliaup_target = get_juliaup_target();

    let new_juliaup_url = format!("https://github.com/JuliaLang/juliaup/releases/download/v{}/juliaup-{}-{}.tar.gz", version, version, juliaup_target);

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

    download_extract_sans_parent(&new_juliaup_url, &my_own_folder, 0)?;

    Ok(())
}
