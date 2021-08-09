mod utils;
mod config_file;
mod versions_file;
mod download;
mod operations;
mod command_add;
mod command_default;
mod command_gc;
mod command_link;
mod command_status;
mod command_remove;
mod command_update;

use crate::command_link::run_command_link;
use crate::command_gc::run_command_gc;
use crate::command_update::run_command_update;
use crate::command_remove::run_command_remove;
use structopt::StructOpt;
use anyhow::{Result};
use command_add::run_command_add;
use command_default::run_command_default;
use command_status::run_command_status;

#[derive(StructOpt)]
#[structopt(
    name="Juliaup - Julia Version Manager"
)]
/// The Julia version manager.
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
    /// Update all or a specific channel to the latest Julia version
    Update {
        channel: Option<String>
    },
    /// Remove a Julia version from your system
    Remove {
        channel: String
    },
    /// Show all installed Julia versions
    Status {
    },
    /// Garbage collect uninstalled Julia versions
    Gc {
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Juliaup::from_args();

    match args {
        Juliaup::Default {channel} => run_command_default(channel),
        Juliaup::Add {channel} => run_command_add(channel).await,
        Juliaup::Remove {channel} => run_command_remove(channel),
        Juliaup::Status {} => run_command_status(),
        Juliaup::Update {channel} => run_command_update(channel).await,
        Juliaup::Gc {} => run_command_gc(),
        Juliaup::Link {channel, file, args} => run_command_link(channel, file, args)
    }
}
