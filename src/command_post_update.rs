use crate::cli::Juliaup;
use crate::command_completions::write_completion_files;
use crate::global_paths::GlobalPaths;
use crate::operations::refresh_existing_shell_init_blocks;
use anyhow::Result;

pub fn run_command_post_update(paths: &GlobalPaths) -> Result<()> {
    // Silently return if we can't determine the bin directory — this runs as a
    // best-effort hook after self-update and should never block the update.
    let bin_path = match std::env::current_exe()?.parent() {
        Some(p) => p.to_path_buf(),
        None => return Ok(()),
    };

    if let Err(e) = write_completion_files::<Juliaup>(&paths.juliauphome, "juliaup") {
        eprintln!("Warning: failed to write completion files: {e}");
    }
    if let Err(e) = refresh_existing_shell_init_blocks(&bin_path, &paths.juliauphome) {
        eprintln!("Warning: failed to refresh shell init blocks: {e}");
    }

    Ok(())
}
