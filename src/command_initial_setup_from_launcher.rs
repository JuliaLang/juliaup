use crate::{command_add::run_command_add, config_file::{load_mut_config_db, save_config_db}, global_paths::GlobalPaths};
use anyhow::{Context, Result};

pub fn run_command_initial_setup_from_launcher(paths: &GlobalPaths) -> Result<()> {
    // Try to add the release channel normally
    match run_command_add("release", paths) {
        Ok(()) => {
            // Success - everything is set up properly
            Ok(())
        }
        Err(_) => {
            // Silently create a minimal config file so we don't keep trying to do initial setup
            // This ensures Julia can start even if the initial installation fails
            let mut config_file = load_mut_config_db(paths)
                .with_context(|| "Failed to create minimal configuration file.")?;
            
            // Just save the config file - this creates the basic structure
            save_config_db(&mut config_file)
                .with_context(|| "Failed to save minimal configuration file.")?;
            
            // Note: We don't print warnings here as this is called from the Julia launcher
            // and users expect Julia to "just work" without juliaup messages
            
            Ok(())
        }
    }
}
