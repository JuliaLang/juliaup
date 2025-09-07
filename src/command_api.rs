use crate::config_file::load_config_db;
use crate::config_file::JuliaupConfigChannel;
use crate::global_paths::GlobalPaths;
use crate::utils::parse_versionstring;
use anyhow::{bail, Context, Result};
use normpath::PathExt;
use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct JuliaupChannelInfo {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "File")]
    pub file: String,
    #[serde(rename = "Args")]
    pub args: Vec<String>,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "Arch")]
    pub arch: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct JuliaupApiGetinfoReturn {
    #[serde(rename = "DefaultChannel")]
    pub default: Option<JuliaupChannelInfo>,
    #[serde(rename = "OtherChannels")]
    pub other_versions: Vec<JuliaupChannelInfo>,
}

pub fn run_command_api(command: &str, paths: &GlobalPaths) -> Result<()> {
    if command != "getconfig1" {
        bail!("Wrong API command.");
    }

    let mut ret_value = JuliaupApiGetinfoReturn {
        default: None,
        other_versions: Vec::new(),
    };

    let config_file = load_config_db(paths, None).with_context(|| {
        "Failed to load configuration file while running the getconfig1 API command."
    })?;

    for (key, value) in &config_file.data.installed_channels {
        let curr = match &value {
            JuliaupConfigChannel::AliasChannel { target } => {
                // Since we no longer support alias-to-alias chains, this is simpler
                JuliaupChannelInfo {
                    name: key.clone(),
                    file: format!("alias-to-{target}"),
                    args: Vec::new(),
                    version: format!("alias to {target}"),
                    arch: String::new(),
                }
            }
            JuliaupConfigChannel::SystemChannel { version: fullversion } => {
                let (platform, mut version) = parse_versionstring(fullversion)
                    .with_context(|| "Encountered invalid version string in the configuration file while running the getconfig1 API command.")?;

                version.build = semver::BuildMetadata::EMPTY;

                match config_file.data.installed_versions.get(fullversion) {
                    Some(channel) => JuliaupChannelInfo {
                        name: key.clone(),
                        file: paths.juliauphome
                            .join(&channel.path)
                            .join("bin")
                            .join(format!("julia{}", std::env::consts::EXE_SUFFIX))
                            .normalize()
                            .with_context(|| "Normalizing the path for an entry from the config file failed while running the getconfig1 API command.")?
                            .into_path_buf()
                            .to_string_lossy()
                            .to_string(),
                        args: Vec::new(),
                        version: version.to_string(),
                        arch: platform
                    },
                    None => bail!("The channel '{}' is configured as a system channel, but no such channel exists in the versions database.", key)
                }
            }
            JuliaupConfigChannel::LinkedChannel { command, args } => {
                let mut new_args = args.as_ref().unwrap_or(&Vec::new()).clone();
                new_args.push("--version".to_string());

                let res = std::process::Command::new(command)
                    .args(&new_args)
                    .output();

                match res {
                    Ok(output) => {
                        let expected_version_prefix = "julia version ";

                        let trimmed_string = std::str::from_utf8(&output.stdout).unwrap().trim();

                        if !trimmed_string.starts_with(expected_version_prefix) {
                            continue;
                        }

                        let version =
                            Version::parse(&trimmed_string[expected_version_prefix.len()..])?;

                        JuliaupChannelInfo {
                            name: key.clone(),
                            file: command.clone(),
                            args: args.as_ref().unwrap_or(&Vec::new()).clone(),
                            version: version.to_string(),
                            arch: String::new(),
                        }
                    }
                    Err(_) => continue,
                }
            }
            JuliaupConfigChannel::DirectDownloadChannel { path, url: _, local_etag: _, server_etag: _, version } => {
                JuliaupChannelInfo {
                    name: key.clone(),
                    file: paths.juliauphome
                        .join(path)
                        .join("bin")
                        .join(format!("julia{}", std::env::consts::EXE_SUFFIX))
                        .normalize()
                        .with_context(|| "Normalizing the path for an entry from the config file failed while running the getconfig1 API command.")?
                        .into_path_buf()
                        .to_string_lossy()
                        .to_string(),
                    args: Vec::new(),
                    version: version.clone(),
                    arch: "".to_string(),
                }
            }
        };

        match config_file.data.default {
            Some(ref default_value) => {
                if key == default_value {
                    ret_value.default = Some(curr.clone());
                } else {
                    ret_value.other_versions.push(curr);
                }
            }
            None => {
                ret_value.other_versions.push(curr);
            }
        }
    }

    // Serialize it to a JSON string.
    let j = serde_json::to_string(&ret_value)?;

    // Print, write to a file, or send to an HTTP server.
    println!("{j}");

    Ok(())
}
