use std::{path::{PathBuf, Path}, env::{current_dir}};

use anyhow::{Result, Context, bail};
use cli_table::{Table, print_stdout, WithTitle, format::{Separator, HorizontalLine, Border}, ColorChoice};
use itertools::Itertools;

use crate::{config_file::{load_config_db, load_mut_config_db, JuliaupOverride, save_config_db}, global_paths::GlobalPaths};

#[derive(Table)]
struct OverrideRow {
    #[table(title = "Path")]
    path: String,
    #[table(title = "Channel")]
    channel: String    
}

pub fn run_command_override_list(paths: &GlobalPaths) -> Result<()> {
    let config_file = load_config_db(paths)
        .with_context(|| "`override list` command failed to load configuration file.")?;

    let rows_in_table: Vec<_> = config_file
        .data
        .overrides
        .iter()
        .sorted_by_key(|i| i.path.to_string())
        .map(|i| -> OverrideRow {
            OverrideRow {
                path: i.path.to_string(),
                channel: i.channel.to_string(),
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

pub fn run_command_override_set(paths: &GlobalPaths, channel: String, path: Option<String>) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`override set` command failed to load configuration data.")?;

    if !config_file.data.installed_channels.contains_key(&channel) {
        bail!("'{}' channel does not exist.", &channel);
    }

    let path = match path {
        Some(path) => PathBuf::from(path),
        None => {
            current_dir()?
        }
    }.canonicalize()?;

    if config_file.data.overrides.iter().any(|i| i.path == path.to_string_lossy().to_string()) {
        bail!("'{}' path already has an override configured.", &channel);
    }
    
    config_file.data.overrides.push(JuliaupOverride { path: path.to_string_lossy().to_string(), channel: channel.clone() });

    save_config_db(&mut config_file)
        .with_context(|| "Failed to save configuration file from `override add` command.")?;

    Ok(())
}

pub fn run_command_override_unset(paths: &GlobalPaths, nonexistent: bool, path: Option<String>) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`override unset` command failed to load configuration data.")?;

    let path = match path {
        Some(path) => PathBuf::from(path),
        None => {
            current_dir()?
        }
    }.canonicalize()?;

    if nonexistent {
        config_file.data.overrides.retain(|x| Path::new(&x.path).is_dir());
    }
    else {
        // First remove any duplicates
        config_file.data.overrides.retain(|x| Path::new(&x.path) != path);
    }

    save_config_db(&mut config_file)
        .with_context(|| "Failed to save configuration file from `override add` command.")?;

    Ok(())
}
