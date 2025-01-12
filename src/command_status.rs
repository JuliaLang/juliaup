use crate::config_file::load_config_db;
use crate::config_file::JuliaupConfigChannel;
use crate::global_paths::GlobalPaths;
use crate::versions_file::load_versions_db;
use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use cli_table::format::HorizontalLine;
use cli_table::format::Separator;
use cli_table::ColorChoice;
use cli_table::{
    format::{Border, Justify},
    print_stdout, Table, WithTitle,
};
use human_sort::compare;
use itertools::Itertools;
use serde::Deserialize;
use serde::Serialize;

#[derive(Table, Serialize, Deserialize)]
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

// First, let's create an enum for the allowed format values
#[derive(ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Json,
    Ndjson,
    Csv,
    Tsv,
}

// Create a separate struct for ListPaths arguments
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Output format (json, ndjson, csv, or tsv)
    #[arg(long, value_enum, short = 'f')]
    pub fmt: Option<OutputFormat>,
}

pub fn run_command_status(paths: &GlobalPaths, fmt: Option<OutputFormat>) -> Result<()> {
    let config_file = load_config_db(paths, None)
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
                    JuliaupConfigChannel::DirectDownloadChannel {
                        path: _,
                        url: _,
                        local_etag: _,
                        server_etag: _,
                        version,
                    } => {
                        format!("Development version {}", version)
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
                    JuliaupConfigChannel::DirectDownloadChannel {
                        path: _,
                        url: _,
                        local_etag,
                        server_etag,
                        version: _,
                    } => {
                        if local_etag != server_etag {
                            "Update available".to_string()
                        } else {
                            "".to_string()
                        }
                    }
                },
            }
        })
        .collect();

    if let Some(f) = fmt {
        match f {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&rows_in_table)?);
            }
            OutputFormat::Ndjson => {
                for row in rows_in_table {
                    println!("{}", serde_json::to_string(&row)?);
                }
            }
            OutputFormat::Csv => {
                println!("default,name,version,update");
                for row in rows_in_table {
                    println!(
                        "{},{},{},{}",
                        row.default, row.name, row.version, row.update
                    );
                }
            }
            OutputFormat::Tsv => {
                println!("default\tname\tversion\tupdate");
                for row in rows_in_table {
                    println!(
                        "{}\t{}\t{}\t{}",
                        row.default, row.name, row.version, row.update
                    );
                }
            }
        }
        return Ok(());
    };

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
