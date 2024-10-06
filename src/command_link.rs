use crate::config_file::JuliaupConfigChannel;
use crate::config_file::{load_mut_config_db, save_config_db};
use crate::global_paths::GlobalPaths;
#[cfg(not(windows))]
use crate::operations::create_symlink;
use crate::utils::is_valid_julia_path;
use crate::versions_file::load_versions_db;
use anyhow::{bail, Context, Result};
use path_absolutize::Absolutize;
use std::path::Path;

pub fn run_command_link(
    channel: &str,
    file: &str,
    args: &[String],
    paths: &GlobalPaths,
) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`link` command failed to load configuration data.")?;

    let versiondb_data =
        load_versions_db(paths).with_context(|| "`link` command failed to load versions db.")?;

    if config_file.data.installed_channels.contains_key(channel) {
        bail!("Channel name `{}` is already used.", channel)
    }

    if versiondb_data.available_channels.contains_key(channel) {
        eprintln!("WARNING: The channel name `{}` is also a system channel. By linking your custom binary to this channel you are hiding this system channel.", channel);
    }

    let absolute_file_path = Path::new(file)
        .absolutize()
        .with_context(|| format!("Failed to convert path `{}` to absolute path.", file))?;

    if !is_valid_julia_path(&absolute_file_path.to_path_buf()) {
        eprintln!("WARNING: There is no julia binary at {}. If this was a mistake, run `juliaup remove {}` and try again.", absolute_file_path.to_string_lossy(), channel);
    }

    config_file.data.installed_channels.insert(
        channel.to_string(),
        JuliaupConfigChannel::LinkedChannel {
            command: absolute_file_path.to_string_lossy().to_string(),
            args: Some(args.to_vec()),
        },
    );

    #[cfg(not(windows))]
    let create_symlinks = config_file.data.settings.create_channel_symlinks;

    save_config_db(&mut config_file)
        .with_context(|| "`link` command failed to save configuration db.")?;

    #[cfg(not(windows))]
    if create_symlinks {
        create_symlink(
            &JuliaupConfigChannel::LinkedChannel {
                command: file.to_string(),
                args: Some(args.to_vec()),
            },
            &format!("julia-{}", channel),
            paths,
        )?;
    }

    Ok(())
}
