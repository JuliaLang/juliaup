use anyhow::{Context, Result};
use clap::Parser;
use juliaup::cli::{ConfigSubCmd, Juliaup, OverrideSubCmd, SelfSubCmd};
use juliaup::command_api::run_command_api;
use juliaup::command_completions::run_command_completions;
#[cfg(not(windows))]
use juliaup::command_config_symlinks::run_command_config_symlinks;
use juliaup::command_config_versionsdbupdate::run_command_config_versionsdbupdate;
use juliaup::command_default::run_command_default;
use juliaup::command_gc::run_command_gc;
use juliaup::command_info::run_command_info;
use juliaup::command_initial_setup_from_launcher::run_command_initial_setup_from_launcher;
use juliaup::command_link::run_command_link;
use juliaup::command_list::run_command_list;
use juliaup::command_override::{run_command_override_status, run_command_override_unset};
use juliaup::command_remove::run_command_remove;
use juliaup::command_selfupdate::run_command_selfupdate;
use juliaup::command_status::run_command_status;
use juliaup::command_update::run_command_update;
use juliaup::command_update_version_db::run_command_update_version_db;
use juliaup::global_paths::get_paths;
use juliaup::{command_add::run_command_add, command_override::run_command_override_set};
#[cfg(feature = "selfupdate")]
use juliaup::{
    command_config_backgroundselfupdate::run_command_config_backgroundselfupdate,
    command_config_modifypath::run_command_config_modifypath,
    command_config_startupselfupdate::run_command_config_startupselfupdate,
    command_selfchannel::run_command_selfchannel,
};

#[cfg(feature = "selfupdate")]
use juliaup::command_selfuninstall::run_command_selfuninstall;

#[cfg(not(feature = "selfupdate"))]
use juliaup::command_selfuninstall::run_command_selfuninstall_unavailable;

use log::info;

fn main() -> Result<()> {
    human_panic::setup_panic!(
        human_panic::Metadata::new("Juliaup", env!("CARGO_PKG_VERSION"))
            .support("https://github.com/JuliaLang/juliaup")
    );

    let env = env_logger::Env::new()
        .filter("JULIAUP_LOG")
        .write_style("JULIAUP_LOG_STYLE");
    env_logger::init_from_env(env);

    #[cfg(feature = "winpkgidentityext")]
    {
        use windows::Management::Deployment::{AddPackageOptions, PackageManager};

        let package_manager = PackageManager::new().unwrap();

        let package_manager_options = AddPackageOptions::new().unwrap();

        let self_location = std::env::current_exe().unwrap();
        let self_location = self_location.parent().unwrap();
        let pkg_loc = self_location.join("juliaup.msix");

        let external_loc =
            windows::Foundation::Uri::CreateUri(&windows::core::HSTRING::from(self_location))
                .unwrap();
        let pkg_loc =
            windows::Foundation::Uri::CreateUri(&windows::core::HSTRING::from(pkg_loc.as_os_str()))
                .unwrap();

        package_manager_options
            .SetExternalLocationUri(&external_loc)
            .unwrap();
        package_manager_options.SetAllowUnsigned(false).unwrap();

        let depl_result = package_manager
            .AddPackageByUriAsync(&pkg_loc, &package_manager_options)
            .unwrap()
            .get()
            .unwrap();

        if !depl_result.IsRegistered().unwrap() {
            eprintln!(
                "Failed to register package identity. Error Message ${:?}",
                depl_result.ErrorText()
            );
        }
    }

    info!("Parsing command line arguments.");
    let args = Juliaup::parse();

    let paths = get_paths().with_context(|| "Trying to load all global paths.")?;

    match args {
        Juliaup::Default { channel } => run_command_default(&channel, &paths),
        Juliaup::Add { channel } => run_command_add(&channel, &paths),
        Juliaup::Remove { channel } => run_command_remove(&channel, &paths),
        Juliaup::Status {} => run_command_status(&paths),
        Juliaup::Update { channel } => run_command_update(channel, &paths),
        Juliaup::Gc {
            prune_linked,
            prune_orphans,
        } => run_command_gc(prune_linked, prune_orphans, &paths),
        Juliaup::Link {
            channel,
            file,
            args,
        } => run_command_link(&channel, &file, &args, &paths),
        Juliaup::List {} => run_command_list(&paths),
        Juliaup::Config(subcmd) => match subcmd {
            #[cfg(not(windows))]
            ConfigSubCmd::ChannelSymlinks { value } => {
                run_command_config_symlinks(value, false, &paths)
            }
            #[cfg(feature = "selfupdate")]
            ConfigSubCmd::BackgroundSelfupdateInterval { value } => {
                run_command_config_backgroundselfupdate(value, false, &paths)
            }
            #[cfg(feature = "selfupdate")]
            ConfigSubCmd::StartupSelfupdateInterval { value } => {
                run_command_config_startupselfupdate(value, false, &paths)
            }
            #[cfg(feature = "selfupdate")]
            ConfigSubCmd::ModifyPath { value } => {
                run_command_config_modifypath(value, false, &paths)
            }
            ConfigSubCmd::VersionsDbUpdateInterval { value } => {
                run_command_config_versionsdbupdate(value, false, &paths)
            }
        },
        Juliaup::Api { command } => run_command_api(&command, &paths),
        Juliaup::InitialSetupFromLauncher {} => run_command_initial_setup_from_launcher(&paths),
        Juliaup::UpdateVersionDb {} => run_command_update_version_db(&paths),
        Juliaup::OverrideSubCmd(subcmd) => match subcmd {
            OverrideSubCmd::Status {} => run_command_override_status(&paths),
            OverrideSubCmd::Set { channel, path } => {
                run_command_override_set(&paths, channel, path)
            }
            OverrideSubCmd::Unset { nonexistent, path } => {
                run_command_override_unset(&paths, nonexistent, path)
            }
        },
        Juliaup::Info {} => run_command_info(&paths),
        #[cfg(feature = "selfupdate")]
        Juliaup::SecretSelfUpdate {} => run_command_selfupdate(&paths),
        Juliaup::SelfSubCmd(subcmd) => match subcmd {
            SelfSubCmd::Update {} => run_command_selfupdate(&paths),
            #[cfg(feature = "selfupdate")]
            SelfSubCmd::Channel { channel } => run_command_selfchannel(channel, &paths),
            #[cfg(feature = "selfupdate")]
            SelfSubCmd::Uninstall {} => run_command_selfuninstall(&paths),
            #[cfg(not(feature = "selfupdate"))]
            SelfSubCmd::Uninstall {} => run_command_selfuninstall_unavailable(),
        },
        Juliaup::Completions { shell } => run_command_completions(shell),
    }
}
