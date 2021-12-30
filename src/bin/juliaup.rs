use juliaup::{command_link::run_command_link};
use juliaup::command_gc::run_command_gc;
use juliaup::command_update::run_command_update;
use juliaup::command_remove::run_command_remove;
use clap::Parser;
use anyhow::{Result};
use juliaup::command_add::run_command_add;
use juliaup::command_default::run_command_default;
use juliaup::command_status::run_command_status;
#[cfg(not(target_os = "windows"))]
use juliaup::command_config_symlinks::run_command_config_symlinks;
use juliaup::command_initial_setup_from_launcher::run_command_initial_setup_from_launcher;
use juliaup::command_api::run_command_api;
#[cfg(feature = "selfupdate")]
use juliaup::{command_selfchannel::run_command_selfchannel,command_selfupdate::run_command_selfupdate,command_selfinstall::run_command_selfinstall, command_selfuninstall::run_command_selfuninstall};
#[cfg(all(not(target_os = "windows"), feature = "selfupdate"))]
use juliaup::command_config_backgroundselfupdate::run_command_config_backgroundselfupdate;
#[cfg(all(not(target_os = "windows"), feature = "selfupdate"))]
use juliaup::command_config_startupselfupdate::run_command_config_startupselfupdate;


#[derive(Parser)]
#[clap(name="Juliaup", version)]
/// The Julia Version Manager
enum Juliaup {
    /// Set the default Julia version
    Default {
        channel: String
    },
    /// Add a specific Julia version or channel to your system
    Add {
        channel: String
    },
    /// Link an existing Julia binary to a custom channel name
    Link {
        channel: String,
        file: String,
        args: Vec<String>
    },
    #[clap(alias="up")]
    /// Update all or a specific channel to the latest Julia version
    Update {
        channel: Option<String>
    },
    #[clap(alias="rm")]
    /// Remove a Julia version from your system
    Remove {
        channel: String
    },
    #[clap(alias="st")]
    /// Show all installed Julia versions
    Status {
    },
    /// Garbage collect uninstalled Julia versions
    Gc {
    },
    #[clap(subcommand, name = "config")]
    /// Juliaup configuration
    Config(ConfigSubCmd),
    #[clap(setting(clap::AppSettings::Hidden))]
    Api {
        command: String
    },
    #[clap(name = "46029ef5-0b73-4a71-bff3-d0d05de42aac", setting(clap::AppSettings::Hidden))]
    InitialSetupFromLauncher {
    },
    #[cfg(feature = "selfupdate")]
    #[clap(subcommand, name = "self")]
    SelfSubCmd(SelfSubCmd),
    // This is used for the cron jobs that we create. By using this UUID for the command
    // We can identify the cron jobs that were created by juliaup for uninstall purposes
    #[cfg(feature = "selfupdate")]
    #[clap(name = "4c79c12db1d34bbbab1f6c6f838f423f", setting(clap::AppSettings::Hidden))]
    SecretSelfUpdate {},
}

#[cfg(feature = "selfupdate")]
#[derive(Parser)]
/// Manage this juliaup installation
enum SelfSubCmd {
    /// Update juliaup itself
    Update {},
    /// Configure the channel to use for juliaup updates
    Channel {
        channel: String
    },
    /// Install this version of juliaup into the system
    Install {},
    /// Uninstall this version of juliaup from the system
    Uninstall {},
}

#[derive(Parser)]
enum ConfigSubCmd {
    #[cfg(not(target_os = "windows"))]
    #[clap(name="channelsymlinks")]
    /// Create a separate symlink per channel
    ChannelSymlinks  {
        /// New Value
        value: Option<bool>
    },
    #[cfg(all(not(target_os = "windows"), feature = "selfupdate"))]
    #[clap(name="backgroundselfupdateinterval")]
    /// The time between automatic background updates of Juliaup in minutes, use 0 to disable.
    BackgroundSelfupdateInterval {
        /// New value
        value: Option<i64>
    },
    #[cfg(all(not(target_os = "windows"), feature = "selfupdate"))]
    #[clap(name="startupselfupdateinterval")]
    /// The time between automatic updates at Julia startup of Juliaup in minutes, use 0 to disable.
    StartupSelfupdateInterval {
        /// New value
        value: Option<i64>
    },
}

fn main() -> Result<()> {
    let args = Juliaup::parse();

    match args {
        Juliaup::Default {channel} => run_command_default(channel),
        Juliaup::Add {channel} => run_command_add(channel),
        Juliaup::Remove {channel} => run_command_remove(channel),
        Juliaup::Status {} => run_command_status(),
        Juliaup::Update {channel} => run_command_update(channel),
        Juliaup::Gc {} => run_command_gc(),
        Juliaup::Link {channel, file, args} => run_command_link(channel, file, args),
        Juliaup::Config(subcmd) => match subcmd {
            #[cfg(not(target_os = "windows"))]
            ConfigSubCmd::ChannelSymlinks {value} => run_command_config_symlinks(value),
            #[cfg(all(not(target_os = "windows"), feature = "selfupdate"))]
            ConfigSubCmd::BackgroundSelfupdateInterval {value} => run_command_config_backgroundselfupdate(value),
            #[cfg(all(not(target_os = "windows"), feature = "selfupdate"))]
            ConfigSubCmd::StartupSelfupdateInterval {value} => run_command_config_startupselfupdate(value),
        },
        Juliaup::Api {command} => run_command_api(command),
        Juliaup::InitialSetupFromLauncher {} => run_command_initial_setup_from_launcher(),
        #[cfg(feature = "selfupdate")]
        Juliaup::SecretSelfUpdate {} => run_command_selfupdate(),
        #[cfg(feature = "selfupdate")]
        Juliaup::SelfSubCmd(subcmd) => match subcmd {
            SelfSubCmd::Update {} => run_command_selfupdate(),
            SelfSubCmd::Channel {channel}  =>  run_command_selfchannel(channel),
            SelfSubCmd::Install {} => run_command_selfinstall(),
            SelfSubCmd::Uninstall {} => run_command_selfuninstall(),
        }
    }
}
