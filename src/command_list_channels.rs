use crate::config_file::load_config_db;
use crate::global_paths::GlobalPaths;
use anyhow::Result;
use itertools::Itertools;

pub fn run_command_list_channels(paths: &GlobalPaths) -> Result<()> {
    // Silently return empty output on error — this is called by shell completion
    // scripts, where any error output or non-zero exit would degrade the user experience.
    let config_file = match load_config_db(paths, None) {
        Ok(cf) => cf,
        Err(_) => return Ok(()),
    };

    for channel_name in config_file.data.installed_channels.keys().sorted() {
        println!("{}", channel_name);
    }

    Ok(())
}
