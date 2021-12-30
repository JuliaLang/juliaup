use crate::operations::garbage_collect_versions;
use crate::config_file::{save_config_db, load_mut_config_db};
use anyhow::{Context, Result};

pub fn run_command_gc() -> Result<()> {
    let mut config_file = load_mut_config_db()
        .with_context(|| "`gc` command failed to load configuration data.")?;

    garbage_collect_versions(&mut config_file.data)?;

    save_config_db(&mut config_file)
        .with_context(|| "`gc` command failed to save configuration db.")?;

    Ok(())
}
