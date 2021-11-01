use juliaup::command_link::run_command_link;
use juliaup::command_gc::run_command_gc;
use juliaup::command_update::run_command_update;
use juliaup::command_remove::run_command_remove;
use clap::Clap;
use anyhow::{Result};
use juliaup::command_add::run_command_add;
use juliaup::command_default::run_command_default;
use juliaup::command_status::run_command_status;
use juliaup::command_initial_setup_from_launcher::run_command_initial_setup_from_launcher;
use juliaup::command_api::run_command_api;
use juliaup::command_selfupdate::run_command_selfupdate;

#[derive(Clap)]
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
    #[clap(setting(clap::AppSettings::Hidden))]
    Api {
        command: String
    },
    #[clap(name = "46029ef5-0b73-4a71-bff3-d0d05de42aac", setting(clap::AppSettings::Hidden))]
    InitialSetupFromLauncher {
    },
    Selfupdate {        
    }
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
        Juliaup::Api {command} => run_command_api(command),
        Juliaup::InitialSetupFromLauncher {} => run_command_initial_setup_from_launcher(),
        Juliaup::Selfupdate {} => run_command_selfupdate(),
    }
}
