use crate::global_paths::GlobalPaths;
use crate::operations::{install_version, update_version_db};
#[cfg(not(windows))]
use crate::operations::create_symlink;
use crate::config_file::{JuliaupConfigChannel, load_mut_config_db, save_config_db};
use crate::versions_file::load_versions_db;
use anyhow::{anyhow, bail, Context, Result};

pub fn run_command_add(channel: &str, paths: &GlobalPaths) -> Result<()> {
    update_version_db(paths)
        .with_context(|| "Failed to update versions db.")?;
    let version_db =
        load_versions_db(paths).with_context(|| "`add` command failed to load versions db.")?;

    let required_version = &version_db
        .available_channels
        .get(channel)
        .ok_or_else(|| anyhow!(
            "'{}' is not a valid Julia version or channel name.",
            &channel
        ))?
        .version;

    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`add` command failed to load configuration data.")?;

    if config_file.data.installed_channels.contains_key(channel) {
        bail!("'{}' is already installed.", &channel);
    }

    #[cfg(all(target_os="macos", target_arch="aarch64"))]
    {
        use crate::utils::parse_versionstring;

        let (platform, _) = parse_versionstring(&required_version)?;

        if platform=="x64" {
            match std::process::Command::new("arch")
                .args(["-x86_64", "/bin/bash", "-c", "arch"])
                .output() {
                    Ok(value) => {
                        if String::from_utf8(value.stdout)? != "i386" {
                            bail!("It seems that you have not yet installed Rosetta, please install it with `softwareupdate --install-rosetta` before you try to install Julia.");
                        }
                        ()
                    },
                    Err(_err) => {
                        bail!("It seems that you have not yet installed Rosetta, please install it with `softwareupdate --install-rosetta` before you try to install Julia.");
                    }
            }
        }
    }
    
    install_version(required_version, &mut config_file.data, &version_db, paths)?;

    config_file.data.installed_channels.insert(
        channel.to_string(),
        JuliaupConfigChannel::SystemChannel {
            version: required_version.clone(),
        },
    );

    if config_file.data.default.is_none() {
        config_file.data.default = Some(channel.to_string());
    }

    #[cfg(not(windows))]
    let create_symlinks = config_file.data.settings.create_channel_symlinks;

    save_config_db(&mut config_file)
        .with_context(|| format!("Failed to save configuration file from `add` command after '{}' was installed.", channel))?;

    #[cfg(not(windows))]
    if create_symlinks {
        create_symlink(
            &JuliaupConfigChannel::SystemChannel {
                version: required_version.clone(),
            },
            &format!("julia-{}", channel),
            paths,
        )?;
    }

    Ok(())
}
