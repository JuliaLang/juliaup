use crate::operations::garbage_collect_versions;
use crate::config_file::{save_config_db, load_mut_config_db, open_mut_config_file};
use anyhow::{Context, Result};

pub fn run_command_gc() -> Result<()> {
    let file = open_mut_config_file()
        .with_context(|| "`gc` command failed to open configuration file.")?;
    
    let (mut config_data, file_lock) = load_mut_config_db(&file)
        .with_context(|| "`gc` command failed to load configuration data.")?;

    garbage_collect_versions(&mut config_data)?;

    save_config_db(&file, config_data, file_lock)
        .with_context(|| "`gc` command failed to save configuration db.")?;

    Ok(())
}
