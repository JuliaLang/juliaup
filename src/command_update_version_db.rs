use crate::{global_paths::GlobalPaths, operations::update_version_db};
use anyhow::{Context, Result};

pub fn run_command_update_version_db(paths: &GlobalPaths) -> Result<()> {
    update_version_db(&None, paths).with_context(|| "Failed to update version db.")?;

    Ok(())
}
