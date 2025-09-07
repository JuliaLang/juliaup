use crate::config_file::load_config_db;
use crate::config_file::JuliaupConfigChannel;
use crate::global_paths::GlobalPaths;
use crate::versions_file::load_versions_db;
use anyhow::{Context, Result};
use cli_table::format::HorizontalLine;
use cli_table::format::Separator;
use cli_table::ColorChoice;
use cli_table::{
    format::{Border, Justify},
    print_stdout, Table, WithTitle,
};
use itertools::Itertools;
use numeric_sort::cmp;

#[derive(Table)]
struct ChannelRow {
    #[table(title = "Default", justify = "Justify::Right")]
    default: &'static str,
    #[table(title = "Channel")]
    name: String,
    #[table(title = "Version")]
    version: String,
    #[table(title = "Update")]
    update: String,
}

pub fn run_command_status(paths: &GlobalPaths) -> Result<()> {
    let config_file = load_config_db(paths, None)
        .with_context(|| "`status` command failed to load configuration file.")?;

    let versiondb_data =
        load_versions_db(paths).with_context(|| "`status` command failed to load versions db.")?;

    let rows_in_table: Vec<_> = config_file
        .data
        .installed_channels
        .iter()
        .sorted_by(|a, b| cmp(&a.0.to_string(), &b.0.to_string()))
        .map(|i| -> ChannelRow {
            ChannelRow {
                default: match config_file.data.default {
                    Some(ref default_value) => {
                        if i.0 == default_value {
                            "*"
                        } else {
                            ""
                        }
                    }
                    None => "",
                },
                name: i.0.to_string(),
                version: match i.1 {
                    JuliaupConfigChannel::SystemChannel { version } => version.clone(),
                    JuliaupConfigChannel::DirectDownloadChannel {
                        path: _,
                        url: _,
                        local_etag: _,
                        server_etag: _,
                        version,
                    } => {
                        format!("Development version {version}")
                    }
                    JuliaupConfigChannel::LinkedChannel { command, args } => {
                        let mut combined_command = String::new();

                        if command.contains(' ') {
                            combined_command.push('\"');
                            combined_command.push_str(command);
                            combined_command.push('\"');
                        } else {
                            combined_command.push_str(command);
                        }

                        if let Some(args) = args {
                            for i in args {
                                combined_command.push(' ');
                                if i.contains(' ') {
                                    combined_command.push('\"');
                                    combined_command.push_str(i);
                                    combined_command.push('\"');
                                } else {
                                    combined_command.push_str(i);
                                }
                            }
                        }
                        format!("Linked to `{combined_command}`")
                    }
                    JuliaupConfigChannel::AliasChannel { target } => {
                        format!("Alias to `{target}`")
                    }
                },
                update: {
                    let update_option = match i.1 {
                        JuliaupConfigChannel::SystemChannel { version } => {
                            match versiondb_data.available_channels.get(i.0) {
                                Some(channel) => {
                                    if &channel.version != version {
                                        Some(format!("Update to {} available", channel.version))
                                    } else {
                                        None
                                    }
                                }
                                None => None,
                            }
                        }
                        JuliaupConfigChannel::LinkedChannel {
                            command: _,
                            args: _,
                        } => None,
                        JuliaupConfigChannel::AliasChannel { target } => {
                            // Check if the target channel has updates available
                            match config_file.data.installed_channels.get(target) {
                                Some(target_channel) => match target_channel {
                                    JuliaupConfigChannel::SystemChannel { version } => {
                                        match versiondb_data.available_channels.get(target) {
                                            Some(channel) => {
                                                if &channel.version != version {
                                                    Some(format!(
                                                        "Update to {} available",
                                                        channel.version
                                                    ))
                                                } else {
                                                    None
                                                }
                                            }
                                            None => None,
                                        }
                                    }
                                    JuliaupConfigChannel::DirectDownloadChannel {
                                        path: _,
                                        url: _,
                                        local_etag,
                                        server_etag,
                                        version: _,
                                    } => {
                                        if local_etag != server_etag {
                                            Some("Update available".to_string())
                                        } else {
                                            None
                                        }
                                    }
                                    // LinkedChannels and nested aliases don't have updates
                                    _ => None,
                                },
                                None => None, // Target channel doesn't exist
                            }
                        }
                        JuliaupConfigChannel::DirectDownloadChannel {
                            path: _,
                            url: _,
                            local_etag,
                            server_etag,
                            version: _,
                        } => {
                            if local_etag != server_etag {
                                Some("Update available".to_string())
                            } else {
                                None
                            }
                        }
                    };
                    update_option.unwrap_or_default()
                },
            }
        })
        .collect();

    print_stdout(
        rows_in_table
            .with_title()
            .color_choice(ColorChoice::Never)
            .border(Border::builder().build())
            .separator(
                Separator::builder()
                    .title(Some(HorizontalLine::new('1', '2', '3', '-')))
                    .build(),
            ),
    )?;

    Ok(())
}
