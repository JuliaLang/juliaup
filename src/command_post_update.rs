use crate::cli::Juliaup;
use crate::command_completions::write_completion_files;
use crate::global_paths::GlobalPaths;
use crate::operations::refresh_existing_shell_init_blocks;
use anyhow::Result;

pub fn run_command_post_update(paths: &GlobalPaths) -> Result<()> {
    let bin_path = std::env::current_exe()?
        .parent()
        .expect("Could not determine parent of current exe")
        .to_path_buf();

    if let Err(e) = write_completion_files::<Juliaup>(&paths.juliauphome, "juliaup") {
        eprintln!("Warning: failed to write completion files: {e}");
    }
    if let Err(e) = refresh_existing_shell_init_blocks(&bin_path, &paths.juliauphome) {
        eprintln!("Warning: failed to refresh shell init blocks: {e}");
    }

    Ok(())
}
