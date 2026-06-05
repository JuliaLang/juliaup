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
    #[cfg(not(windows))]
    if let Err(e) = restore_symlinks(&bin_path, paths) {
        eprintln!("Warning: failed to restore Julia symlinks: {e}");
    }

    Ok(())
}

// Self-update replaces the entire bin directory, which removes the `julia`
// symlink (and any channel symlinks) that are not part of the juliaup tarball.
// Recreate them here so `julia` keeps working after an update.
#[cfg(not(windows))]
fn restore_symlinks(bin_path: &std::path::Path, paths: &GlobalPaths) -> Result<()> {
    use crate::config_file::load_config_db;
    use crate::operations::create_symlink;
    use anyhow::Context;

    let launcher_path = bin_path.join("julialauncher");
    if launcher_path.exists() {
        let julia_symlink = bin_path.join("julia");
        // Remove any stale or dangling link before recreating it.
        let _ = std::fs::remove_file(&julia_symlink);
        std::os::unix::fs::symlink(&launcher_path, &julia_symlink).with_context(|| {
            format!(
                "failed to create symlink `{}`.",
                julia_symlink.to_string_lossy()
            )
        })?;
    }

    let config_file = load_config_db(paths, None)
        .with_context(|| "Failed to load configuration db while restoring symlinks.")?;
    if config_file.data.settings.create_channel_symlinks {
        for (channel_name, channel) in &config_file.data.installed_channels {
            create_symlink(channel, &format!("julia-{}", channel_name), paths)?;
        }
    }

    Ok(())
}
