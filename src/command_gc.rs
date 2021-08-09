use crate::operations::garbage_collect_versions;
use crate::config_file::{load_config_db, save_config_db};
use anyhow::{Context, Result};

pub fn run_command_gc() -> Result<()> {
    let mut config_data = load_config_db()
        .with_context(|| "`gc` command failed to load configuration file.")?;

    garbage_collect_versions(&mut config_data)?;

    save_config_db(&config_data)
        .with_context(|| "`gc` command failed to save configuration db.")?;

    Ok(())
}
