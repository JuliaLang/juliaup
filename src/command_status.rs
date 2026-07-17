use crate::config_file::load_config_db;
use crate::config_file::{JuliaupConfigChannel, JuliaupReadonlyConfigFile};
use crate::global_paths::GlobalPaths;
use crate::jsonstructs_versionsdb::JuliaupVersionDB;
use crate::utils::format_version_for_display;
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
use regex::Regex;

fn format_linked_command(command: &str, args: &Option<Vec<String>>) -> String {
    let mut combined_command = String::new();

    if command.contains(' ') {
        combined_command.push('\"');
        combined_command.push_str(command);
        combined_command.push('\"');
    } else {
        combined_command.push_str(command);
    }

    if let Some(args) = args {
        for arg in args {
            combined_command.push(' ');
            if arg.contains(' ') {
                combined_command.push('\"');
                combined_command.push_str(arg);
                combined_command.push('\"');
            } else {
                combined_command.push_str(arg);
            }
        }
    }

    format!("Linked to `{combined_command}`")
}

fn format_version(channel_name: &str, channel: &JuliaupConfigChannel) -> String {
    match channel {
        JuliaupConfigChannel::DirectDownloadChannel { version, .. } => {
            let version = format_version_for_display(version);
            match Regex::new(r"^pr(\d+)").unwrap().captures(channel_name) {
                Some(caps) => format!(
                    "{version} https://github.com/JuliaLang/julia/pull/{}",
                    &caps[1]
                ),
                None => version,
            }
        }
        JuliaupConfigChannel::SystemChannel { version } => format_version_for_display(version),
        JuliaupConfigChannel::LinkedChannel { command, args } => {
            format_linked_command(command, args)
        }
        JuliaupConfigChannel::AliasChannel { target, args } => match args {
            Some(args) if !args.is_empty() => {
                format!("Alias to `{target}` with args: {:?}", args)
            }
            _ => format!("Alias to `{target}`"),
        },
    }
}

fn get_update_info(
    channel_name: &str,
    channel: &JuliaupConfigChannel,
    config_file: &JuliaupReadonlyConfigFile,
    versiondb_data: &JuliaupVersionDB,
) -> String {
    match channel {
        JuliaupConfigChannel::DirectDownloadChannel {
            local_etag,
            server_etag,
            ..
        } => (local_etag != server_etag).then(|| "Update available".to_string()),
        JuliaupConfigChannel::SystemChannel { version } => {
            match versiondb_data.available_channels.get(channel_name) {
                Some(channel) if &channel.version != version => Some(format!(
                    "Update to {} available",
                    format_version_for_display(&channel.version)
                )),
                _ => None,
            }
        }
        JuliaupConfigChannel::LinkedChannel { .. } => None,
        JuliaupConfigChannel::AliasChannel { target, .. } => {
            // Check if the target channel has updates available
            match config_file.data.installed_channels.get(target) {
                Some(JuliaupConfigChannel::DirectDownloadChannel {
                    local_etag,
                    server_etag,
                    ..
                }) => (local_etag != server_etag).then(|| "Update available".to_string()),
                Some(JuliaupConfigChannel::SystemChannel { version }) => {
                    match versiondb_data.available_channels.get(target) {
                        Some(channel) if channel.version != *version => Some(format!(
                            "Update to {} available",
                            format_version_for_display(&channel.version)
                        )),
                        _ => None,
                    }
                }
                _ => None, // Target channel doesn't exist or not updatable
            }
        }
    }
    .unwrap_or_default()
}

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

    let rows_in_table: Vec<ChannelRow> = config_file
        .data
        .installed_channels
        .iter()
        .sorted_by(|(channel_name_a, _), (channel_name_b, _)| cmp(channel_name_a, channel_name_b))
        .map(|(channel_name, channel)| ChannelRow {
            default: match &config_file.data.default {
                Some(ref default_value) if channel_name == default_value => "*",
                _ => "",
            },
            name: channel_name.to_string(),
            version: format_version(channel_name, channel),
            update: get_update_info(channel_name, channel, &config_file, &versiondb_data),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn direct_download_channel(version: &str) -> JuliaupConfigChannel {
        JuliaupConfigChannel::DirectDownloadChannel {
            path: "somepath".to_string(),
            url: "https://example.com/julia.tar.gz".to_string(),
            local_etag: "etag".to_string(),
            server_etag: "etag".to_string(),
            version: version.to_string(),
            binary_path: None,
        }
    }

    #[test]
    fn format_version_pr_channel_shows_pr_url() {
        assert_eq!(
            format_version("pr59158", &direct_download_channel("1.14.0-DEV.123")),
            "1.14.0-DEV.123 https://github.com/JuliaLang/julia/pull/59158"
        );
    }

    #[test]
    fn format_version_pr_channel_with_platform_suffix_shows_pr_url() {
        assert_eq!(
            format_version("pr59158~x64", &direct_download_channel("1.14.0-DEV.123")),
            "1.14.0-DEV.123 https://github.com/JuliaLang/julia/pull/59158"
        );
    }

    #[test]
    fn format_version_nightly_channel_shows_only_version() {
        assert_eq!(
            format_version("nightly", &direct_download_channel("1.14.0-DEV.456")),
            "1.14.0-DEV.456"
        );
        assert_eq!(
            format_version("1.13-nightly", &direct_download_channel("1.13.0-DEV.789")),
            "1.13.0-DEV.789"
        );
    }

    #[test]
    fn format_version_system_channel_drops_build_metadata() {
        assert_eq!(
            format_version(
                "1.11",
                &JuliaupConfigChannel::SystemChannel {
                    version: "1.11.2+0.x64.apple.darwin14".to_string()
                }
            ),
            "1.11.2"
        );
    }
}
