use crate::config_file::{load_mut_config_db, save_config_db};
use crate::global_paths::GlobalPaths;
use crate::operations::garbage_collect_versions;
use anyhow::{Context, Result};

pub fn run_command_gc(prune_linked: bool, paths: &GlobalPaths) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`gc` command failed to load configuration data.")?;

    garbage_collect_versions(prune_linked, &mut config_file.data, paths)?;

    save_config_db(&mut config_file)
        .with_context(|| "`gc` command failed to save configuration db.")?;

    Ok(())
}
