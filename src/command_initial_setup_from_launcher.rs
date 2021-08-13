use crate::operations::install_version;
use crate::versions_file::load_versions_db;
use crate::config_file::save_config_db;
use crate::config_file::{JuliaupConfigVersion,JuliaupConfigChannel};
use std::collections::HashMap;
use crate::config_file::JuliaupConfig;
use crate::utils::get_juliaup_home_path;
use crate::utils::get_arch;
use crate::get_bundled_julia_full_version;
use anyhow::{Context, Result};
use std::path::Path;

pub fn run_command_initial_setup_from_launcher() -> Result<()> {
    let juliaup_folder = get_juliaup_home_path()?;

    let my_own_path = std::env::current_exe()?;

    let path_of_bundled_version = my_own_path
        .parent()
        .unwrap() // unwrap OK because we can't get a path that does not have a parent
        .join("BundledJulia");

    let platform = get_arch()?;

    let full_version_string = format!("{}~{}", get_bundled_julia_full_version(), platform);

    if path_of_bundled_version.exists() {
        let target_folder_name = format!("julia-{}", full_version_string);
        let target_path = juliaup_folder.join(&target_folder_name);

        std::fs::create_dir_all(&target_path)?;

        let mut options = fs_extra::dir::CopyOptions::new();
        options.overwrite = true;
        options.content_only = true;
        fs_extra::dir::copy(path_of_bundled_version, target_path, &options)?;
        
        let mut juliaup_confi_data = JuliaupConfig {
            default: Some("release".to_string()),
            installed_versions: HashMap::new(),
            installed_channels: HashMap::new(),
        };

        juliaup_confi_data.installed_versions.insert(
            full_version_string.clone(),
            JuliaupConfigVersion {
                path: Path::new(".")
                    .join(&target_folder_name)
                    .display()
                    .to_string(),
            },
        );

        juliaup_confi_data.installed_channels.insert(
            "release".to_string(),
            JuliaupConfigChannel::SystemChannel {
                version: full_version_string.clone(),
            },
        );
        save_config_db(&juliaup_confi_data)?;
    } else {
        let mut juliaup_confi_data = JuliaupConfig {
            default: Some("release".to_string()),
            installed_versions: HashMap::new(),
            installed_channels: HashMap::new(),
        };

        juliaup_confi_data.installed_channels.insert(
            "release".to_string(),
            JuliaupConfigChannel::SystemChannel {
                version: full_version_string.clone(),
            },
        );

        let version_db =
            load_versions_db().with_context(|| "`update` command failed to load versions db.")?;

        std::fs::create_dir_all(juliaup_folder)?;

        install_version(&full_version_string, &mut juliaup_confi_data, &version_db)?;

        save_config_db(&juliaup_confi_data)?;
    }
    Ok(())
}
