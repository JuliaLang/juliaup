use clap::Parser;

#[derive(Parser)]
#[clap(name = "Juliaup", version)]
#[command(
    after_help = "To launch a specific Julia version, use `julia +{channel}` e.g. `julia +1.6`.
Entering just `julia` uses the default channel set via `juliaup default`."
)]
/// The Julia Version Manager
pub enum Juliaup {
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
    /// Generate tab-completion scripts for your shell
    Completions { shell: String },
    // This is used for the cron jobs that we create. By using this UUID for the command
    // We can identify the cron jobs that were created by juliaup for uninstall purposes
    #[cfg(feature = "selfupdate")]
    #[clap(name = "4c79c12db1d34bbbab1f6c6f838f423f", hide = true)]
    SecretSelfUpdate {},
}

#[derive(Parser)]
/// Manage directory overrides
pub enum OverrideSubCmd {
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
pub enum SelfSubCmd {
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
    #[cfg(not(feature = "selfupdate"))]
    /// Uninstall this version of juliaup from the system (UNAVAILABLE)
    Uninstall {},
}

#[derive(Parser)]
pub enum ConfigSubCmd {
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
    /// The time between automatic updates of the nightly Julia version in minutes, use 0 to disable.
    #[clap(name = "nightlyupdateinterval")]
    NightlyUpdateInterval {
        /// New value
        value: Option<i64>,
    },
}
