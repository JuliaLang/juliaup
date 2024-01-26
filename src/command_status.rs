use crate::config_file::load_config_db;
use crate::config_file::JuliaupConfigChannel;
use crate::global_paths::GlobalPaths;
use crate::versions_file::load_versions_db;
use anyhow::{Context, Result};
use chrono::Utc;
use cli_table::format::HorizontalLine;
use cli_table::format::Separator;
use cli_table::ColorChoice;
use cli_table::{
    format::{Border, Justify},
    print_stdout, Table, WithTitle,
};
use human_sort::compare;
use itertools::Itertools;

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
    let config_file = load_config_db(paths)
        .with_context(|| "`status` command failed to load configuration file.")?;

    let versiondb_data =
        load_versions_db(paths).with_context(|| "`status` command failed to load versions db.")?;

    let rows_in_table: Vec<_> = config_file
        .data
        .installed_channels
        .iter()
        .sorted_by(|a, b| compare(&a.0.to_string(), &b.0.to_string()))
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
                    JuliaupConfigChannel::DirectDownloadChannel { path, url, last_update, version } => {
                        // let last_update = config_file
                        //     .data
                        //     .installed_versions
                        //     .get(nightly_version)
                        //     .unwrap()
                        //     .last_update;
                        // let now = Utc::now();
                        // let duration = now.signed_duration_since(last_update);
                        // let days_old = duration.num_days();
                        // format!("{} ({} days old)", nightly_version, days_old)
                        format!("FOOO") // TODO FIX
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
                        format!("Linked to `{}`", combined_command)
                    }
                },
                update: match i.1 {
                    JuliaupConfigChannel::SystemChannel { version } => {
                        match versiondb_data.available_channels.get(i.0) {
                            Some(channel) => {
                                if &channel.version != version {
                                    format!("Update to {} available", channel.version)
                                } else {
                                    "".to_string()
                                }
                            }
                            None => "".to_string(),
                        }
                    }
                    JuliaupConfigChannel::LinkedChannel {
                        command: _,
                        args: _,
                    } => "".to_string(),
                    JuliaupConfigChannel::DirectDownloadChannel { path, url, last_update, version } => "".to_string(),
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
