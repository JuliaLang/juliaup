use anyhow::{Context, Result};
use clap::Parser;
use juliaup::command_api::run_command_api;
#[cfg(not(windows))]
use juliaup::command_config_symlinks::run_command_config_symlinks;
use juliaup::command_config_versionsdbupdate::run_command_config_versionsdbupdate;
use juliaup::command_config_checkchanneluptodate::run_command_config_checkchanneluptodate;
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
    command_selfchannel::run_command_selfchannel, command_selfuninstall::run_command_selfuninstall,
};
use log::info;

#[derive(Parser)]
#[clap(name = "Juliaup", version)]
/// The Julia Version Manager
enum Juliaup {
    /// Set the default Julia version
    Default { channel: String },
    /// Add a specific Julia version or channel to your system. Access via `julia +{channel}` e.g. `julia +1.6`
    Add { channel: String },
    /// Link an existing Julia binary to a custom channel name
    Link {
        channel: String,
        file: String,
        args: Vec<String>,
    },
    /// List all available channels
    #[clap(alias = "ls")]
    List {},
    #[clap(subcommand, name = "override")]
    OverrideSubCmd(OverrideSubCmd),
    #[clap(alias = "up")]
    /// Update all or a specific channel to the latest Julia version
    Update { channel: Option<String> },
    #[clap(alias = "rm")]
    /// Remove a Julia version from your system
    Remove { channel: String },
    #[clap(alias = "st")]
    /// Show all installed Julia versions
    Status {},
    /// Garbage collect uninstalled Julia versions
    Gc {},
    #[clap(subcommand, name = "config")]
    /// Juliaup configuration
    Config(ConfigSubCmd),
    #[clap(hide = true)]
    Api { command: String },
    #[clap(name = "46029ef5-0b73-4a71-bff3-d0d05de42aac", hide = true)]
    InitialSetupFromLauncher {},
    #[clap(name = "0cf1528f-0b15-46b1-9ac9-e5bf5ccccbcf", hide = true)]
    UpdateVersionDb {},
    #[clap(name = "info", hide = true)]
    Info {},
    #[clap(subcommand, name = "self")]
    SelfSubCmd(SelfSubCmd),
    // This is used for the cron jobs that we create. By using this UUID for the command
    // We can identify the cron jobs that were created by juliaup for uninstall purposes
    #[cfg(feature = "selfupdate")]
    #[clap(name = "4c79c12db1d34bbbab1f6c6f838f423f", hide = true)]
    SecretSelfUpdate {},
}

#[derive(Parser)]
/// Manage directory overrides
enum OverrideSubCmd {
    Status {},
    Set {
        channel: String,
        #[clap(long, short)]
        path: Option<String>,
    },
    Unset {
        #[clap(long, short)]
        nonexistent: bool,
        #[clap(long, short)]
        path: Option<String>,
    },
}

#[derive(Parser)]
/// Manage this juliaup installation
enum SelfSubCmd {
    #[cfg(not(feature = "selfupdate"))]
    /// Update the Julia versions database
    Update {},
    #[cfg(feature = "selfupdate")]
    /// Update the Julia versions database and juliaup itself
    Update {},
    #[cfg(feature = "selfupdate")]
    /// Configure the channel to use for juliaup updates
    Channel { channel: String },
    #[cfg(feature = "selfupdate")]
    /// Uninstall this version of juliaup from the system
    Uninstall {},
}

#[derive(Parser)]
enum ConfigSubCmd {
    #[cfg(not(windows))]
    #[clap(name = "channelsymlinks")]
    /// Create a separate symlink per channel
    ChannelSymlinks {
        /// New Value
        value: Option<bool>,
    },
    #[cfg(feature = "selfupdate")]
    #[clap(name = "backgroundselfupdateinterval")]
    /// The time between automatic background updates of Juliaup in minutes, use 0 to disable.
    BackgroundSelfupdateInterval {
        /// New value
        value: Option<i64>,
    },
    #[cfg(feature = "selfupdate")]
    #[clap(name = "startupselfupdateinterval")]
    /// The time between automatic updates at Julia startup of Juliaup in minutes, use 0 to disable.
    StartupSelfupdateInterval {
        /// New value
        value: Option<i64>,
    },
    #[cfg(feature = "selfupdate")]
    #[clap(name = "modifypath")]
    /// Add the Julia binaries to your PATH by manipulating various shell startup scripts.
    ModifyPath {
        /// New value
        value: Option<bool>,
    },
    /// The time between automatic updates of the versions database in minutes, use 0 to disable.
    #[clap(name = "versionsdbupdateinterval")]
    VersionsDbUpdateInterval {
        /// New value
        value: Option<i64>,
    },
    #[clap(name = "checkchanneluptodate")]
    /// Check for updates to the default channel on startup
    ShouldCheckChannelUptodate {
        /// New Value
        value: Option<bool>,
    },
}

fn main() -> Result<()> {
    human_panic::setup_panic!(human_panic::Metadata {
        name: "Juliaup".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        authors: "".into(),
        homepage: "https://github.com/JuliaLang/juliaup".into(),
    });

    let env = env_logger::Env::new()
        .filter("JULIAUP_LOG")
        .write_style("JULIAUP_LOG_STYLE");
    env_logger::init_from_env(env);

    info!("Parsing command line arguments.");
    let args = Juliaup::parse();

    let paths = get_paths().with_context(|| "Trying to load all global paths.")?;

    match args {
        Juliaup::Default { channel } => run_command_default(&channel, &paths),
        Juliaup::Add { channel } => run_command_add(&channel, &paths),
        Juliaup::Remove { channel } => run_command_remove(&channel, &paths),
        Juliaup::Status {} => run_command_status(&paths),
        Juliaup::Update { channel } => run_command_update(channel, &paths),
        Juliaup::Gc {} => run_command_gc(&paths),
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
            ConfigSubCmd::ShouldCheckChannelUptodate { value } => {
                run_command_config_checkchanneluptodate(value, false, &paths)
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
        },
    }
}
