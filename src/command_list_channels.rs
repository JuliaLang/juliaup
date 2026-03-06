use crate::config_file::load_config_db;
use crate::global_paths::GlobalPaths;
use anyhow::{Context, Result};
use itertools::Itertools;

pub fn run_command_list_channels(paths: &GlobalPaths) -> Result<()> {
    let config_file = load_config_db(paths, None)
        .with_context(|| "Failed to load configuration file for listing channels.")?;

    for channel_name in config_file.data.installed_channels.keys().sorted() {
        println!("{}", channel_name);
    }

    Ok(())
}
