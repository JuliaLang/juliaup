#[cfg(not(windows))]
use crate::operations::remove_symlink;
use crate::utils::{print_juliaup_style, JuliaupMessageType};
use crate::{
    config_file::{load_mut_config_db, save_config_db, JuliaupConfig, JuliaupConfigChannel},
    global_paths::GlobalPaths,
    operations::garbage_collect_versions,
};
use anyhow::{bail, Context, Result};

fn is_pattern(arg: &str) -> bool {
    arg.contains(['*', '?'])
}

fn pattern_to_regex(pattern: &str) -> regex::Regex {
    let mut re = String::from("^");
    for c in pattern.chars() {
        match c {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            _ => re.push_str(&regex::escape(&c.to_string())),
        }
    }
    re.push('$');
    regex::Regex::new(&re).unwrap()
}

fn removal_blocker(config: &JuliaupConfig, channel: &str) -> Option<&'static str> {
    if config.default.as_deref() == Some(channel) {
        Some("it is currently configured as the default channel")
    } else if config.overrides.iter().any(|i| i.channel == channel) {
        Some("it is currently used in a directory override")
    } else {
        None
    }
}

fn resolve_channels(config: &JuliaupConfig, args: &[String]) -> Result<Vec<String>> {
    let mut channels: Vec<String> = Vec::new();

    for arg in args {
        if is_pattern(arg) {
            let re = pattern_to_regex(arg);
            let mut matches: Vec<&String> = config
                .installed_channels
                .keys()
                .filter(|k| re.is_match(k))
                .collect();
            matches.sort();

            if matches.is_empty() {
                bail!(
                    "No installed channels match '{}'. Please run `juliaup status` to see installed channels.",
                    arg
                );
            }

            for channel in matches {
                // A pattern can easily catch the default or an overridden channel by accident,
                // so skip those instead of failing the whole command.
                if let Some(reason) = removal_blocker(config, channel) {
                    print_juliaup_style(
                        "Skipping",
                        &format!("'{channel}' because {reason}."),
                        JuliaupMessageType::Warning,
                    );
                } else if !channels.contains(channel) {
                    channels.push(channel.clone());
                }
            }
        } else {
            if !config.installed_channels.contains_key(arg) {
                bail!(
                    "'{}' cannot be removed because it is not currently installed. Please run `juliaup list` to see available channels.",
                    arg
                );
            }

            if let Some(reason) = removal_blocker(config, arg) {
                bail!("'{}' cannot be removed because {}.", arg, reason);
            }

            if !channels.contains(arg) {
                channels.push(arg.clone());
            }
        }
    }

    Ok(channels)
}

pub fn run_command_remove(args: &[String], paths: &GlobalPaths) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`remove` command failed to load configuration data.")?;

    // Check every argument before removing anything, so a typo does not leave a partial removal.
    let channels = resolve_channels(&config_file.data, args)?;

    let mut messages = Vec::new();

    for channel in &channels {
        let channel_info = config_file.data.installed_channels.get(channel).unwrap();

        // Determine what type of channel is being removed for better messaging
        let channel_type = match channel_info {
            JuliaupConfigChannel::DirectDownloadChannel { .. } => "channel".to_string(),
            JuliaupConfigChannel::SystemChannel { .. } => "channel".to_string(),
            JuliaupConfigChannel::LinkedChannel { .. } => "linked channel".to_string(),
            JuliaupConfigChannel::AliasChannel { target, .. } => {
                format!("alias (pointing to '{target}')")
            }
        };

        if let JuliaupConfigChannel::DirectDownloadChannel {
            path,
            url: _,
            local_etag: _,
            server_etag: _,
            version: _,
            binary_path: _,
        } = channel_info
        {
            let path_to_delete = paths.juliauphome.join(path);

            let display = path_to_delete.display();

            if let Err(e) = std::fs::remove_dir_all(&path_to_delete) {
                eprintln!(
                    "WARNING: Failed to delete {display}: {e}. You can try to delete at a later point by running `juliaup gc`."
                )
            }
        };

        config_file.data.installed_channels.remove(channel);

        #[cfg(not(windows))]
        remove_symlink(&format!("julia-{channel}"))?;

        messages.push(format!(
            "Julia {channel_type} '{channel}' successfully removed."
        ));
    }

    garbage_collect_versions(false, &mut config_file.data, paths)?;

    save_config_db(&mut config_file, paths).with_context(|| {
        format!(
            "Failed to save configuration file from `remove` command after removing {} at `{}`.",
            channels
                .iter()
                .map(|c| format!("'{c}'"))
                .collect::<Vec<_>>()
                .join(", "),
            paths.juliaupconfig.display()
        )
    })?;

    for message in messages {
        print_juliaup_style("Remove", &message, JuliaupMessageType::Success);
    }

    Ok(())
}
