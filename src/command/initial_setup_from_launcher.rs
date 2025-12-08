use crate::{command, global_paths::GlobalPaths};
use anyhow::{Context, Result};

pub fn run(paths: &GlobalPaths) -> Result<()> {
    command::add("release", paths)
        .with_context(|| "Failed to run `run_command_add` from the `run_command_initial_setup_from_launcher` command.")?;

    Ok(())
}
