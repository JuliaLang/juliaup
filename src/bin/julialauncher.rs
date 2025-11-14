use anyhow::{anyhow, bail, Context, Result};
use console::{style, Term};
use dialoguer::Select;
use is_terminal::IsTerminal;
use itertools::Itertools;
use juliaup::config_file::{
    load_config_db, load_mut_config_db, save_config_db, JuliaupConfig, JuliaupConfigChannel,
};
use juliaup::global_paths::get_paths;
use juliaup::jsonstructs_versionsdb::JuliaupVersionDB;
use juliaup::operations::{is_pr_channel, is_valid_channel};
use juliaup::utils::{print_juliaup_style, JuliaupMessageType};
use juliaup::version_selection::get_auto_channel;
use juliaup::versions_file::load_versions_db;
#[cfg(not(windows))]
use nix::{
    sys::wait::{waitpid, WaitStatus},
    unistd::{fork, ForkResult},
};
use normpath::PathExt;
#[cfg(not(windows))]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, RawHandle};
use std::path::Path;
use std::path::PathBuf;
#[cfg(windows)]
use windows::Win32::System::{
    JobObjects::{AssignProcessToJobObject, SetInformationJobObject},
    Threading::GetCurrentProcess,
};

#[derive(thiserror::Error, Debug)]
#[error("{msg}")]
pub struct UserError {
    msg: String,
}

fn get_juliaup_path() -> Result<PathBuf> {
    let my_own_path = std::env::current_exe()
        .with_context(|| "std::env::current_exe() did not find its own path.")?
        .canonicalize()
        .with_context(|| "Failed to canonicalize the path to the Julia launcher.")?;

    let juliaup_path = my_own_path
        .parent()
        .unwrap() // unwrap OK here because this can't happen
        .join(format!("juliaup{}", std::env::consts::EXE_SUFFIX));

    Ok(juliaup_path)
}

fn do_initial_setup(juliaupconfig_path: &Path) -> Result<()> {
    if !juliaupconfig_path.exists() {
        let juliaup_path = get_juliaup_path().with_context(|| "Failed to obtain juliaup path.")?;

        std::process::Command::new(juliaup_path)
            .arg("46029ef5-0b73-4a71-bff3-d0d05de42aac") // This is our internal command to do the initial setup
            .status()
            .with_context(|| "Failed to start juliaup for the initial setup.")?;
    }
    Ok(())
}

fn run_versiondb_update(
    config_file: &juliaup::config_file::JuliaupReadonlyConfigFile,
) -> Result<()> {
    use chrono::Utc;
    use std::process::Stdio;

    let versiondb_update_interval = config_file.data.settings.versionsdb_update_interval;

    if versiondb_update_interval > 0 {
        let should_run =
            if let Some(last_versiondb_update) = config_file.data.last_version_db_update {
                let update_time =
                    last_versiondb_update + chrono::Duration::minutes(versiondb_update_interval);
                Utc::now() >= update_time
            } else {
                true
            };

        if should_run {
            let juliaup_path =
                get_juliaup_path().with_context(|| "Failed to obtain juliaup path.")?;

            std::process::Command::new(juliaup_path)
                .args(["0cf1528f-0b15-46b1-9ac9-e5bf5ccccbcf"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .spawn()
                .with_context(|| "Failed to start juliaup for version db update.")?;
        };
    }

    Ok(())
}

#[cfg(feature = "selfupdate")]
fn run_selfupdate(config_file: &juliaup::config_file::JuliaupReadonlyConfigFile) -> Result<()> {
    use chrono::Utc;
    use std::process::Stdio;

    if let Some(val) = config_file.self_data.startup_selfupdate_interval {
        let should_run = if let Some(last_selfupdate) = config_file.self_data.last_selfupdate {
            let update_time = last_selfupdate + chrono::Duration::minutes(val);

            Utc::now() >= update_time
        } else {
            true
        };

        if should_run {
            let juliaup_path =
                get_juliaup_path().with_context(|| "Failed to obtain juliaup path.")?;

            std::process::Command::new(juliaup_path)
                .args(["self", "update"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .spawn()
                .with_context(|| "Failed to start juliaup for self update.")?;
        };
    }

    Ok(())
}

#[cfg(not(feature = "selfupdate"))]
fn run_selfupdate(_config_file: &juliaup::config_file::JuliaupReadonlyConfigFile) -> Result<()> {
    Ok(())
}

fn is_interactive() -> bool {
    // First check if we have TTY access - this is a prerequisite for interactivity
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return false;
    }

    // Even with TTY available, check if Julia is being invoked in a non-interactive way
    let args: Vec<String> = std::env::args().collect();

    // Skip the first argument (program name) and any channel specification (+channel)
    let mut julia_args = args.iter().skip(1);

    // Skip channel specification if present
    if let Some(first_arg) = julia_args.clone().next() {
        if first_arg.starts_with('+') {
            julia_args.next(); // consume the +channel argument
        }
    }

    // Check for non-interactive usage patterns
    for arg in julia_args {
        match arg.as_str() {
            // Expression evaluation is non-interactive
            "-e" | "--eval" | "-E" | "--print" => return false,
            // Reading from stdin pipe is non-interactive
            "-" => return false,
            // Version display is non-interactive
            "-v" | "--version" => return false,
            // Help is non-interactive
            "-h" | "--help" | "--help-hidden" => return false,
            // Check if this looks like a Julia file (ends with .jl)
            filename if filename.ends_with(".jl") && !filename.starts_with('-') => {
                return false;
            }
            // Any other non-flag argument that doesn't start with '-' could be a script
            filename if !filename.starts_with('-') && !filename.is_empty() => {
                // This could be a script file, check if it exists as a file
                if std::path::Path::new(filename).exists() {
                    return false;
                }
            }
            _ => {} // Continue checking other arguments
        }
    }

    true
}

fn handle_auto_install_prompt(
    channel: &str,
    paths: &juliaup::global_paths::GlobalPaths,
) -> Result<bool> {
    // Check if we're in interactive mode
    if !is_interactive() {
        // Non-interactive mode, don't auto-install
        return Ok(false);
    }

    // Use dialoguer for a consistent UI experience
    let selection = Select::new()
        .with_prompt(format!(
            "{} The Juliaup channel '{}' is not installed. Would you like to install it?",
            style("Question:").yellow().bold(),
            channel
        ))
        .item("Yes (install this time only)")
        .item("Yes and remember my choice (always auto-install)")
        .item("No")
        .default(0) // Default to "Yes"
        .interact()?;

    match selection {
        0 => {
            // Just install for this time
            Ok(true)
        }
        1 => {
            // Install and remember the preference
            set_auto_install_preference(true, paths)?;
            Ok(true)
        }
        2 => {
            // Don't install
            Ok(false)
        }
        _ => {
            // Should not happen with dialoguer, but default to no
            Ok(false)
        }
    }
}

fn set_auto_install_preference(
    auto_install: bool,
    paths: &juliaup::global_paths::GlobalPaths,
) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "Failed to load configuration for setting auto-install preference.")?;

    config_file.data.settings.auto_install_channels = Some(auto_install);

    save_config_db(&mut config_file)
        .with_context(|| "Failed to save auto-install preference to configuration.")?;

    print_juliaup_style(
        "Configure",
        &format!("Auto-install preference set to '{}'.", auto_install),
        JuliaupMessageType::Success,
    );

    Ok(())
}

fn spawn_juliaup_add(
    channel: &str,
    _paths: &juliaup::global_paths::GlobalPaths,
    is_automatic: bool,
) -> Result<()> {
    if is_automatic {
        print_juliaup_style(
            "Installing",
            &format!("Julia {} automatically per juliaup settings", channel),
            JuliaupMessageType::Progress,
        );
    } else {
        print_juliaup_style(
            "Installing",
            &format!("Julia {} as requested", channel),
            JuliaupMessageType::Progress,
        );
    }

    let juliaup_path = get_juliaup_path().with_context(|| "Failed to obtain juliaup path.")?;

    let status = std::process::Command::new(juliaup_path)
        .args(["add", channel])
        .status()
        .with_context(|| format!("Failed to spawn juliaup to install channel '{}'", channel))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "Failed to install channel '{}'. juliaup add command failed with exit code: {:?}",
            channel,
            status.code()
        ))
    }
}

fn check_channel_uptodate(
    channel: &str,
    current_version: &str,
    versions_db: &JuliaupVersionDB,
) -> Result<()> {
    let latest_version = &versions_db
        .available_channels
        .get(channel)
        .ok_or_else(|| UserError {
            msg: format!(
                "The channel `{}` does not exist in the versions database.",
                channel
            ),
        })?
        .version;

    if latest_version != current_version {
        print_juliaup_style(
            "Info",
            &format!(
                "The latest version of Julia in the `{}` channel is {}. You currently have `{}` installed. Run:",
                channel, latest_version, current_version
            ),
            JuliaupMessageType::Progress,
        );
        eprintln!();
        eprintln!("  juliaup update");
        eprintln!();
        eprintln!(
            "in your terminal shell to install Julia {} and update the `{}` channel to that version.",
            latest_version, channel
        );
    }
    Ok(())
}

fn is_nightly_channel(channel: &str) -> bool {
    use regex::Regex;
    let nightly_re =
        Regex::new(r"^((?:nightly|latest)|(\d+\.\d+)-(?:nightly|latest))(~|$)").unwrap();
    nightly_re.is_match(channel)
}

#[derive(Debug)]
enum JuliaupChannelSource {
    CmdLine,
    EnvVar,
    Override,
    Auto,
    Default,
}

fn get_julia_path_from_channel(
    versions_db: &JuliaupVersionDB,
    config_data: &JuliaupConfig,
    channel: &str,
    juliaupconfig_path: &Path,
    juliaup_channel_source: JuliaupChannelSource,
    paths: &juliaup::global_paths::GlobalPaths,
) -> Result<(PathBuf, Vec<String>)> {
    // First check if the channel is an alias and extract its args
    let (resolved_channel, alias_args) = match config_data.installed_channels.get(channel) {
        Some(JuliaupConfigChannel::AliasChannel { target, args }) => {
            (target.to_string(), args.clone().unwrap_or_default())
        }
        _ => (channel.to_string(), Vec::new()),
    };

    let channel_valid = is_valid_channel(versions_db, &resolved_channel)?;

    // First check if the channel is already installed
    if let Some(channel_info) = config_data.installed_channels.get(&resolved_channel) {
        return get_julia_path_from_installed_channel(
            versions_db,
            config_data,
            &resolved_channel,
            juliaupconfig_path,
            channel_info,
            alias_args.clone(),
        );
    }

    // Handle auto-installation for command line channel selection and auto-resolved channels
    if matches!(
        juliaup_channel_source,
        JuliaupChannelSource::CmdLine | JuliaupChannelSource::Auto
    ) && (channel_valid
        || is_pr_channel(&resolved_channel)
        || is_nightly_channel(&resolved_channel))
    {
        // Check the user's auto-install preference
        let should_auto_install = match config_data.settings.auto_install_channels {
            Some(auto_install) => auto_install, // User has explicitly set a preference
            None => {
                // User hasn't set a preference - prompt in interactive mode, default to false in non-interactive
                if is_interactive() {
                    handle_auto_install_prompt(&resolved_channel, paths)?
                } else {
                    false
                }
            }
        };

        if should_auto_install {
            // Install the channel using juliaup
            let is_automatic = config_data.settings.auto_install_channels == Some(true);
            spawn_juliaup_add(&resolved_channel, paths, is_automatic)?;

            // Reload the config to get the newly installed channel
            let updated_config_file = load_config_db(paths, None)
                .with_context(|| "Failed to reload configuration after installing channel.")?;

            let updated_channel_info = updated_config_file
                .data
                .installed_channels
                .get(&resolved_channel);

            if let Some(channel_info) = updated_channel_info {
                return get_julia_path_from_installed_channel(
                    versions_db,
                    &updated_config_file.data,
                    &resolved_channel,
                    juliaupconfig_path,
                    channel_info,
                    alias_args,
                );
            } else {
                return Err(anyhow!(
                        "Channel '{resolved_channel}' was installed but could not be found in configuration."
                    ));
            }
        }
        // If we reach here, either installation failed or user declined
    }

    // Original error handling for non-command-line sources or invalid channels
    let error = match juliaup_channel_source {
        JuliaupChannelSource::CmdLine => {
            if channel_valid {
                UserError { msg: format!("`{resolved_channel}` is not installed. Please run `juliaup add {resolved_channel}` to install channel or version.") }
            } else if is_pr_channel(&resolved_channel) {
                UserError { msg: format!("`{resolved_channel}` is not installed. Please run `juliaup add {resolved_channel}` to install pull request channel if available.") }
            } else if is_nightly_channel(&resolved_channel) {
                UserError { msg: format!("`{resolved_channel}` is not installed. Please run `juliaup add {resolved_channel}` to install nightly channel.") }
            } else {
                UserError { msg: format!("Invalid Juliaup channel `{resolved_channel}`. Please run `juliaup list` to get a list of valid channels and versions.") }
            }
        },
        JuliaupChannelSource::EnvVar=> {
            if channel_valid {
                UserError { msg: format!("`{resolved_channel}` from environment variable JULIAUP_CHANNEL is not installed. Please run `juliaup add {resolved_channel}` to install channel or version.") }
            } else if is_pr_channel(&resolved_channel) {
                UserError { msg: format!("`{resolved_channel}` from environment variable JULIAUP_CHANNEL is not installed. Please run `juliaup add {resolved_channel}` to install pull request channel if available.") }
            } else {
                UserError { msg: format!("Invalid Juliaup channel `{resolved_channel}` from environment variable JULIAUP_CHANNEL. Please run `juliaup list` to get a list of valid channels and versions.") }
            }
        },
        JuliaupChannelSource::Override=> {
            if channel_valid {
                UserError { msg: format!("`{resolved_channel}` from directory override is not installed. Please run `juliaup add {resolved_channel}` to install channel or version.") }
            } else if is_pr_channel(&resolved_channel) {
                UserError { msg: format!("`{resolved_channel}` from directory override is not installed. Please run `juliaup add {resolved_channel}` to install pull request channel if available.") }
            } else {
                UserError { msg: format!("Invalid Juliaup channel `{resolved_channel}` from directory override. Please run `juliaup list` to get a list of valid channels and versions.") }
            }
        },
        JuliaupChannelSource::Auto => {
            if channel_valid {
                UserError { msg: format!("`{resolved_channel}` resolved from project manifest is not installed. Please run `juliaup add {resolved_channel}` to install channel or version.") }
            } else if is_pr_channel(&resolved_channel) {
                UserError { msg: format!("`{resolved_channel}` resolved from project manifest is not installed. Please run `juliaup add {resolved_channel}` to install pull request channel if available.") }
            } else if is_nightly_channel(&resolved_channel) {
                UserError { msg: format!("`{resolved_channel}` resolved from project manifest is not installed. Please run `juliaup add {resolved_channel}` to install nightly channel.") }
            } else {
                UserError { msg: format!("Invalid Juliaup channel `{resolved_channel}` resolved from project manifest. Please run `juliaup list` to get a list of valid channels and versions.") }
            }
        },
        JuliaupChannelSource::Default => UserError {msg: format!("The Juliaup configuration is in an inconsistent state, the currently configured default channel `{resolved_channel}` is not installed.") }
    };

    Err(error.into())
}

fn get_julia_path_from_installed_channel(
    versions_db: &JuliaupVersionDB,
    config_data: &JuliaupConfig,
    channel: &str,
    juliaupconfig_path: &Path,
    channel_info: &JuliaupConfigChannel,
    alias_args: Vec<String>,
) -> Result<(PathBuf, Vec<String>)> {
    match channel_info {
        JuliaupConfigChannel::AliasChannel { .. } => {
            bail!("Unexpected alias channel after resolution: {channel}");
        }
        JuliaupConfigChannel::LinkedChannel { command, args } => {
            let mut combined_args = alias_args;
            combined_args.extend(args.as_ref().map_or_else(Vec::new, |v| v.clone()));
            Ok((PathBuf::from(command), combined_args))
        }
        JuliaupConfigChannel::SystemChannel { version } => {
            let path = &config_data
                .installed_versions.get(version)
                .ok_or_else(|| anyhow!("The juliaup configuration is in an inconsistent state, the channel {channel} is pointing to Julia version {version}, which is not installed."))?.path;

            if is_interactive() {
                check_channel_uptodate(channel, version, versions_db).with_context(|| {
                    format!("The Julia launcher failed while checking whether the channel {channel} is up-to-date.")
                })?;
            }

            let absolute_path = juliaupconfig_path
                .parent()
                .unwrap() // unwrap OK because there should always be a parent
                .join(path)
                .join("bin")
                .join(format!("julia{}", std::env::consts::EXE_SUFFIX))
                .normalize()
                .with_context(|| {
                    format!(
                        "Failed to normalize path for Julia binary, starting from `{}`.",
                        juliaupconfig_path.display()
                    )
                })?;
            Ok((absolute_path.into_path_buf(), alias_args))
        }
        JuliaupConfigChannel::DirectDownloadChannel {
            path,
            url: _,
            local_etag,
            server_etag,
            version: _,
        } => {
            if local_etag != server_etag && is_interactive() {
                if channel.starts_with("nightly") {
                    // Nightly is updateable several times per day so this message will show
                    // more often than not unless folks update a couple of times a day.
                    // Also, folks using nightly are typically more experienced and need
                    // less detailed prompting
                    print_juliaup_style(
                        "Info",
                        "A new `nightly` version is available. Install with `juliaup update`.",
                        JuliaupMessageType::Progress,
                    );
                } else {
                    print_juliaup_style(
                        "Info",
                        &format!(
                            "A new version of Julia for the `{}` channel is available. Run:",
                            channel
                        ),
                        JuliaupMessageType::Progress,
                    );
                    eprintln!();
                    eprintln!("  juliaup update");
                    eprintln!();
                    eprintln!("to install the latest Julia for the `{}` channel.", channel);
                }
            }

            let absolute_path = juliaupconfig_path
                .parent()
                .unwrap()
                .join(path)
                .join("bin")
                .join(format!("julia{}", std::env::consts::EXE_SUFFIX))
                .normalize()
                .with_context(|| {
                    format!(
                        "Failed to normalize path for Julia binary, starting from `{}`.",
                        juliaupconfig_path.display()
                    )
                })?;
            Ok((absolute_path.into_path_buf(), alias_args))
        }
    }
}

fn get_override_channel(
    config_file: &juliaup::config_file::JuliaupReadonlyConfigFile,
) -> Result<Option<String>> {
    let curr_dir = std::env::current_dir()?.canonicalize()?;

    let juliaup_override = config_file
        .data
        .overrides
        .iter()
        .filter(|i| curr_dir.starts_with(&i.path))
        .sorted_by_key(|i| i.path.len())
        .next_back();

    match juliaup_override {
        Some(val) => Ok(Some(val.channel.clone())),
        None => Ok(None),
    }
}

/// Determines which channel to use based on the inputs.
/// This is the core channel selection logic used both in production and tests.
/// Returns (channel_name, source)
///
/// Priority order:
/// 1. Command line (+channel)
/// 2. Environment variable (JULIAUP_CHANNEL)
/// 3. Override (from config file)
/// 4. Auto-detection (from project manifest)
/// 5. Default (from config file)
fn determine_channel(
    args: &[String],
    env_channel: Option<String>,
    override_channel: Option<String>,
    default_channel: Option<String>,
    versions_db: &JuliaupVersionDB,
    manifest_version_detect: bool,
) -> Result<(String, JuliaupChannelSource)> {
    // Parse command line for +channel
    let mut channel_from_cmd_line: Option<String> = None;
    if args.len() > 1 {
        let first_arg = &args[1];
        if let Some(stripped) = first_arg.strip_prefix('+') {
            channel_from_cmd_line = Some(stripped.to_string());
        }
    }

    // Priority order
    if let Some(channel) = channel_from_cmd_line {
        Ok((channel, JuliaupChannelSource::CmdLine))
    } else if let Some(channel) = env_channel {
        Ok((channel, JuliaupChannelSource::EnvVar))
    } else if let Some(channel) = override_channel {
        Ok((channel, JuliaupChannelSource::Override))
    } else if let Some(channel) = get_auto_channel(args, versions_db, manifest_version_detect)? {
        Ok((channel, JuliaupChannelSource::Auto))
    } else if let Some(channel) = default_channel {
        Ok((channel, JuliaupChannelSource::Default))
    } else {
        Err(anyhow!("Failed to determine juliaup channel"))
    }
}

fn run_app() -> Result<i32> {
    if std::io::stdout().is_terminal() {
        // Set console title
        let term = Term::stdout();
        term.set_title("Julia");
    }

    let paths = get_paths().with_context(|| "Trying to load all global paths.")?;

    do_initial_setup(&paths.juliaupconfig)
        .with_context(|| "The Julia launcher failed to run the initial setup steps.")?;

    let config_file = load_config_db(&paths, None)
        .with_context(|| "The Julia launcher failed to load a configuration file.")?;

    let versiondb_data = load_versions_db(&paths)
        .with_context(|| "The Julia launcher failed to load a versions db.")?;

    // Determine which channel to use
    let args: Vec<String> = std::env::args().collect();
    let (julia_channel_to_use, juliaup_channel_source) = determine_channel(
        &args,
        std::env::var("JULIAUP_CHANNEL").ok(),
        get_override_channel(&config_file)?,
        config_file.data.default.clone(),
        &versiondb_data,
        config_file.data.settings.manifest_version_detect,
    )
    .with_context(|| "The Julia launcher failed to figure out which juliaup channel to use.")?;

    let (julia_path, julia_args) = get_julia_path_from_channel(
        &versiondb_data,
        &config_file.data,
        &julia_channel_to_use,
        &paths.juliaupconfig,
        juliaup_channel_source,
        &paths,
    )
    .with_context(|| {
        format!(
            "The Julia launcher failed to determine the command for the `{}` channel.",
            julia_channel_to_use
        )
    })?;

    let mut new_args: Vec<String> = Vec::new();

    for i in julia_args {
        new_args.push(i);
    }

    for (i, v) in args.iter().skip(1).enumerate() {
        if i > 0 || !v.starts_with('+') {
            new_args.push(v.clone());
        }
    }

    // On *nix platforms we replace the current process with the Julia one.
    // This simplifies use in e.g. debuggers, but requires that we fork off
    // a subprocess to do the selfupdate and versiondb update.
    #[cfg(not(windows))]
    match unsafe { fork() } {
        // NOTE: It is unsafe to perform async-signal-unsafe operations from
        // forked multithreaded programs, so for complex functionality like
        // selfupdate to work julialauncher needs to remain single-threaded.
        // Ref: https://docs.rs/nix/latest/nix/unistd/fn.fork.html#safety
        Ok(ForkResult::Parent { child, .. }) => {
            // wait for the daemon-spawning child to finish
            match waitpid(child, None) {
                Ok(WaitStatus::Exited(_, code)) => {
                    if code != 0 {
                        panic!("Could not fork (child process exited with code: {})", code)
                    }
                }
                Ok(_) => {
                    panic!("Could not fork (child process did not exit normally)");
                }
                Err(e) => {
                    panic!("Could not fork (error waiting for child process, {})", e);
                }
            }

            // replace the current process
            let _ = std::process::Command::new(&julia_path)
                .args(&new_args)
                .exec();

            // this is only ever reached if launching Julia fails
            panic!(
                "Could not launch Julia. Verify that there is a valid Julia binary at \"{}\".",
                julia_path.to_string_lossy()
            )
        }
        Ok(ForkResult::Child) => {
            // double-fork to prevent zombies
            match unsafe { fork() } {
                Ok(ForkResult::Parent { child: _, .. }) => {
                    // we don't do anything here so that this process can be
                    // reaped immediately
                }
                Ok(ForkResult::Child) => {
                    // this is where we perform the actual work. we don't do
                    // any typical daemon-y things (like detaching the TTY)
                    // so that any error output is still visible.

                    // We set a Ctrl-C handler here that just doesn't do anything, as we want the Julia child
                    // process to handle things.
                    ctrlc::set_handler(|| ())
                        .with_context(|| "Failed to set the Ctrl-C handler.")?;

                    run_versiondb_update(&config_file)
                        .with_context(|| "Failed to run version db update")?;

                    run_selfupdate(&config_file).with_context(|| "Failed to run selfupdate.")?;
                }
                Err(_) => panic!("Could not double-fork"),
            }

            Ok(0)
        }
        Err(_) => panic!("Could not fork"),
    }

    // On other platforms (i.e., Windows) we just spawn a subprocess
    #[cfg(windows)]
    {
        // We set a Ctrl-C handler here that just doesn't do anything, as we want the Julia child
        // process to handle things.
        ctrlc::set_handler(|| ()).with_context(|| "Failed to set the Ctrl-C handler.")?;

        let mut job_attr: windows::Win32::Security::SECURITY_ATTRIBUTES =
            windows::Win32::Security::SECURITY_ATTRIBUTES::default();
        let mut job_info: windows::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION =
            windows::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();

        job_attr.bInheritHandle = false.into();
        job_info.BasicLimitInformation.LimitFlags =
            windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_BREAKAWAY_OK
                | windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK
                | windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let job_handle = unsafe {
            windows::Win32::System::JobObjects::CreateJobObjectW(
                Some(&job_attr),
                windows::core::PCWSTR::null(),
            )
        }?;
        unsafe {
            SetInformationJobObject(
                job_handle,
                windows::Win32::System::JobObjects::JobObjectExtendedLimitInformation,
                &job_info as *const _ as *const std::os::raw::c_void,
                std::mem::size_of_val(&job_info) as u32,
            )
        }?;

        unsafe { AssignProcessToJobObject(job_handle, GetCurrentProcess()) }?;

        let mut child_process = std::process::Command::new(julia_path)
            .args(&new_args)
            .spawn()
            .with_context(|| "The Julia launcher failed to start Julia.")?; // TODO Maybe include the command we actually tried to start?

        // We ignore any error here, as that is what libuv also does, see the documentation
        // at https://github.com/libuv/libuv/blob/5ff1fc724f7f53d921599dbe18e6f96b298233f1/src/win/process.c#L1077
        let _ = unsafe {
            AssignProcessToJobObject(
                job_handle,
                std::mem::transmute::<RawHandle, windows::Win32::Foundation::HANDLE>(
                    child_process.as_raw_handle(),
                ),
            )
        };

        run_versiondb_update(&config_file).with_context(|| "Failed to run version db update")?;

        run_selfupdate(&config_file).with_context(|| "Failed to run selfupdate.")?;

        let status = child_process
            .wait()
            .with_context(|| "Failed to wait for Julia process to finish.")?;

        let code = match status.code() {
            Some(code) => code,
            None => {
                anyhow::bail!("There is no exit code, that should not be possible on Windows.");
            }
        };

        Ok(code)
    }
}

fn main() -> Result<std::process::ExitCode> {
    let client_status: std::prelude::v1::Result<i32, anyhow::Error>;

    {
        human_panic::setup_panic!(human_panic::Metadata::new(
            "Juliaup launcher",
            env!("CARGO_PKG_VERSION")
        )
        .support("https://github.com/JuliaLang/juliaup"));

        let env = env_logger::Env::new()
            .filter("JULIAUP_LOG")
            .write_style("JULIAUP_LOG_STYLE");
        env_logger::init_from_env(env);

        client_status = run_app();

        if let Err(err) = &client_status {
            if let Some(e) = err.downcast_ref::<UserError>() {
                eprintln!("{} {}", style("ERROR:").red().bold(), e.msg);

                return Ok(std::process::ExitCode::FAILURE);
            } else {
                return Err(client_status.unwrap_err());
            }
        }
    }

    // TODO https://github.com/rust-lang/rust/issues/111688 is finalized, we should use that instead of calling exit
    std::process::exit(client_status?);
}

#[cfg(test)]
mod tests {
    use super::*;
    use juliaup::jsonstructs_versionsdb::{JuliaupVersionDBChannel, JuliaupVersionDBVersion};
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    // Helper to create a test directory with Project.toml
    fn create_test_project(dir: &Path, project_content: &str) -> PathBuf {
        let project_file = dir.join("Project.toml");
        fs::write(&project_file, project_content).unwrap();
        project_file
    }

    // Helper to create a manifest file
    fn create_manifest(dir: &Path, name: &str, julia_version: &str) {
        let manifest_file = dir.join(name);
        fs::write(
            &manifest_file,
            format!(r#"julia_version = "{}""#, julia_version),
        )
        .unwrap();
    }

    // Helper to create a project with a standard manifest in one call
    fn create_project_with_manifest(dir: &Path, julia_version: &str) -> PathBuf {
        let project_file = create_test_project(dir, "name = \"TestProject\"");
        create_manifest(dir, "Manifest.toml", julia_version);
        project_file
    }

    // Helper to build julia args, optionally with --project flag
    // Pass None for no project flag, Some(path) for --project={path}
    fn julia_args(project_path: Option<&Path>) -> Vec<String> {
        let mut args = vec!["julia".to_string()];
        if let Some(path) = project_path {
            args.push(format!("--project={}", path.display()));
        }
        args.push("-e".to_string());
        args.push("1+1".to_string());
        args
    }

    // Helper to build julia args with --project flag (no value, searches upward)
    fn julia_args_with_project_search() -> Vec<String> {
        vec![
            "julia".to_string(),
            "--project".to_string(),
            "-e".to_string(),
            "1+1".to_string(),
        ]
    }

    // Helper to assert channel determination result
    fn assert_channel(
        result: Result<(String, JuliaupChannelSource)>,
        expected_channel: &str,
        expected_source: JuliaupChannelSource,
    ) {
        assert!(result.is_ok());
        let (channel, source) = result.unwrap();
        assert_eq!(channel, expected_channel);
        // Use discriminant comparison to check enum variant equality
        assert_eq!(
            std::mem::discriminant(&source),
            std::mem::discriminant(&expected_source),
            "Expected source {:?}, got {:?}",
            expected_source,
            source
        );
    }

    // Helper to build a test versions database
    struct TestVersionsDbBuilder {
        available_versions: HashMap<String, JuliaupVersionDBVersion>,
        available_channels: HashMap<String, JuliaupVersionDBChannel>,
    }

    impl TestVersionsDbBuilder {
        fn new() -> Self {
            Self {
                available_versions: HashMap::new(),
                available_channels: HashMap::new(),
            }
        }

        fn add_version(mut self, version: &str) -> Self {
            self.available_versions.insert(
                version.to_string(),
                JuliaupVersionDBVersion {
                    url_path: "test".to_string(),
                },
            );
            self
        }

        fn add_channel(mut self, channel: &str, version: &str) -> Self {
            self.available_channels.insert(
                channel.to_string(),
                JuliaupVersionDBChannel {
                    version: version.to_string(),
                },
            );
            self
        }

        fn build(self) -> JuliaupVersionDB {
            JuliaupVersionDB {
                available_versions: self.available_versions,
                available_channels: self.available_channels,
                version: "1".to_string(),
            }
        }
    }

    // Helper to create a minimal versions db for testing
    fn create_test_versions_db() -> JuliaupVersionDB {
        TestVersionsDbBuilder::new()
            .add_version("1.10.0")
            .add_channel("1.10.0", "1.10.0")
            .add_version("1.10.5")
            .add_channel("1.10.5", "1.10.5")
            .add_version("1.11.0")
            .add_channel("1.11.0", "1.11.0")
            .add_version("1.11.3")
            .add_channel("1.11.3", "1.11.3")
            .build()
    }

    // Integration tests for determine_channel - tests the full channel selection logic
    #[test]
    fn test_channel_selection_priority_cmdline_wins() {
        // Test that +channel has highest priority
        let temp_dir = TempDir::new().unwrap();
        create_project_with_manifest(temp_dir.path(), "1.10.5");

        let args = vec![
            "julia".to_string(),
            "+1.11.3".to_string(),
            format!("--project={}", temp_dir.path().display()),
            "-e".to_string(),
            "1+1".to_string(),
        ];

        let versions_db = create_test_versions_db();
        let result = determine_channel(
            &args,
            Some("1.10.0".to_string()),
            Some("override".to_string()),
            Some("default".to_string()),
            &versions_db,
            true,
        );

        assert_channel(result, "1.11.3", JuliaupChannelSource::CmdLine);
    }

    #[test]
    fn test_channel_selection_priority_env_over_auto() {
        // Test that JULIAUP_CHANNEL has priority over auto-detected version
        let temp_dir = TempDir::new().unwrap();
        create_project_with_manifest(temp_dir.path(), "1.10.5");
        let args = julia_args(Some(temp_dir.path()));

        let versions_db = create_test_versions_db();
        let result = determine_channel(
            &args,
            Some("1.11.3".to_string()),
            None,
            Some("default".to_string()),
            &versions_db,
            true,
        );

        assert_channel(result, "1.11.3", JuliaupChannelSource::EnvVar);
    }

    #[test]
    fn test_channel_selection_auto_from_manifest() {
        // Test that auto-detection works when no higher priority source
        let temp_dir = TempDir::new().unwrap();
        create_project_with_manifest(temp_dir.path(), "1.10.5");
        let args = julia_args(Some(temp_dir.path()));

        let versions_db = create_test_versions_db();
        let result = determine_channel(
            &args,
            None,
            None,
            Some("default".to_string()),
            &versions_db,
            true,
        );

        assert_channel(result, "1.10.5", JuliaupChannelSource::Auto);
    }

    #[test]
    fn test_channel_selection_default_fallback() {
        // Test that default channel is used when nothing else applies
        let args = julia_args(None);

        let versions_db = create_test_versions_db();
        let result = determine_channel(
            &args,
            None,
            None,
            Some("release".to_string()),
            &versions_db,
            true,
        );

        assert_channel(result, "release", JuliaupChannelSource::Default);
    }

    #[test]
    fn test_channel_selection_override_priority() {
        // Test that override has priority over auto and default
        let temp_dir = TempDir::new().unwrap();
        create_project_with_manifest(temp_dir.path(), "1.10.5");
        let args = julia_args(Some(temp_dir.path()));

        let versions_db = create_test_versions_db();
        let result = determine_channel(
            &args,
            None,
            Some("1.11.0".to_string()),
            Some("default".to_string()),
            &versions_db,
            true,
        );

        assert_channel(result, "1.11.0", JuliaupChannelSource::Override);
    }

    #[test]
    fn test_channel_selection_auto_with_project_flag_no_value() {
        // Test auto-detection with --project (no value) searches upward
        let temp_dir = TempDir::new().unwrap();
        create_project_with_manifest(temp_dir.path(), "1.11.0");

        // Change to temp directory for the test
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let args = julia_args_with_project_search();

        let versions_db = create_test_versions_db();
        let result = determine_channel(
            &args,
            None,
            None,
            Some("default".to_string()),
            &versions_db,
            true,
        );

        // Restore directory
        std::env::set_current_dir(old_dir).unwrap();

        assert_channel(result, "1.11.0", JuliaupChannelSource::Auto);
    }
}
