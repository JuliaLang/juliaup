use crate::{command_add::run_command_add, global_paths::GlobalPaths};
use anyhow::{Context, Result};

pub fn run_command_initial_setup_from_launcher(paths: &GlobalPaths) -> Result<()> {
    run_command_add("release".to_string(), paths)
        .with_context(|| "Failed to run `run_command_add` from the `run_command_initial_setup_from_launcher` command.")?;

    Ok(())
}
