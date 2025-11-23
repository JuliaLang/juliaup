use anyhow::{Context, Result};
use clap::Parser;
use juliaup::cli::{ConfigSubCmd, Juliaup, OverrideSubCmd, SelfSubCmd};
use juliaup::command;
use juliaup::global_paths::get_paths;

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

    #[cfg(all(windows, feature = "winpkgidentityext"))]
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
        Juliaup::Default { channel } => command::default(&channel, &paths),
        Juliaup::Add { channel } => command::add(&channel, &paths),
        Juliaup::Remove { channel } => command::remove(&channel, &paths),
        Juliaup::Status {} => command::status(&paths),
        Juliaup::Update { channel } => command::update(&channel, &paths),
        Juliaup::Gc { prune_linked } => command::gc(prune_linked, &paths),
        Juliaup::Link {
            channel,
            target,
            args,
        } => command::link(&channel, &target, &args, &paths),
        Juliaup::List {} => command::list(&paths),
        Juliaup::Config(subcmd) => match subcmd {
            #[cfg(not(windows))]
            ConfigSubCmd::ChannelSymlinks { value } => {
                command::config::symlinks(value, false, &paths)
            }
            #[cfg(feature = "selfupdate")]
            ConfigSubCmd::BackgroundSelfupdateInterval { value } => {
                command::config::background_self_update(value, false, &paths)
            }
            #[cfg(feature = "selfupdate")]
            ConfigSubCmd::StartupSelfupdateInterval { value } => {
                command::config::startup_self_update(value, false, &paths)
            }
            #[cfg(feature = "selfupdate")]
            ConfigSubCmd::ModifyPath { value } => {
                command::config::modify_path(value, false, &paths)
            }
            ConfigSubCmd::VersionsDbUpdateInterval { value } => {
                command::config::versionsdb_update(value, false, &paths)
            }
            ConfigSubCmd::AutoInstallChannels { value } => {
                command::config::autoinstall(value, false, &paths)
            }
        },
        Juliaup::Api { command } => command::api(&command, &paths),
        Juliaup::InitialSetupFromLauncher {} => command::initial_setup_from_launcher(&paths),
        Juliaup::UpdateVersionDb {} => command::update_versiondb(&paths),
        Juliaup::OverrideSubCmd(subcmd) => match subcmd {
            OverrideSubCmd::Status {} => command::r#override::status(&paths),
            OverrideSubCmd::Set { channel, path } => {
                command::r#override::set(&paths, channel, path)
            }
            OverrideSubCmd::Unset { nonexistent, path } => {
                command::r#override::unset(&paths, nonexistent, path)
            }
        },
        Juliaup::Info {} => command::info(&paths),
        #[cfg(feature = "selfupdate")]
        Juliaup::SecretSelfUpdate {} => command::selfupdate(&paths),
        Juliaup::SelfSubCmd(subcmd) => match subcmd {
            SelfSubCmd::Update {} => command::selfupdate(&paths),
            #[cfg(feature = "selfupdate")]
            SelfSubCmd::Channel { channel } => command::selfchannel(channel, &paths),
            #[cfg(feature = "selfupdate")]
            SelfSubCmd::Uninstall {} => command::selfuninstall(&paths),
            #[cfg(not(feature = "selfupdate"))]
            SelfSubCmd::Uninstall {} => command::selfuninstall::unavailable(),
        },
        Juliaup::Completions { shell } => {
            command::completions::generate::<Juliaup>(shell, "juliaup")
        }
    }
}
