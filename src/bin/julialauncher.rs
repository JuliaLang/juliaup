use anyhow::{anyhow, Context, Result};
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
            "-e" | "--eval" => return false,
            // Reading from stdin pipe is non-interactive
            "-" => return false,
            // Print options are typically non-interactive
            "-p" | "--print" => return false,
            // License/version display is non-interactive
            "-L" | "--license" => return false,
            "-v" | "--version" => return false,
            // Help is non-interactive
            "-h" | "--help" => return false,
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

    eprintln!(
        "{} Auto-install preference set to '{}'.",
        style("Info:").cyan().bold(),
        auto_install
    );

    Ok(())
}

fn spawn_juliaup_add(
    channel: &str,
    _paths: &juliaup::global_paths::GlobalPaths,
    is_automatic: bool,
) -> Result<()> {
    if is_automatic {
        eprintln!(
            "{} Installing Julia {} automatically per juliaup settings...",
            style("Info:").cyan().bold(),
            channel
        );
    } else {
        eprintln!(
            "{} Installing Julia {} as requested...",
            style("Info:").cyan().bold(),
            channel
        );
    }

    let juliaup_path = get_juliaup_path().with_context(|| "Failed to obtain juliaup path.")?;

    let status = std::process::Command::new(juliaup_path)
        .args(["add", channel])
        .status()
        .with_context(|| format!("Failed to spawn juliaup to install channel '{}'", channel))?;

    if status.success() {
        eprintln!(
            "{} Successfully installed Julia {}.",
            style("Info:").cyan().bold(),
            channel
        );
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
        eprintln!("The latest version of Julia in the `{}` channel is {}. You currently have `{}` installed. Run:", channel, latest_version, current_version);
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

enum JuliaupChannelSource {
    CmdLine,
    EnvVar,
    Override,
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
    let channel_valid = is_valid_channel(versions_db, &channel.to_string())?;

    // First check if the channel is already installed
    if let Some(channel_info) = config_data.installed_channels.get(channel) {
        return get_julia_path_from_installed_channel(
            versions_db,
            config_data,
            channel,
            juliaupconfig_path,
            channel_info,
        );
    }

    // Handle auto-installation for command line channel selection
    if let JuliaupChannelSource::CmdLine = juliaup_channel_source {
        if channel_valid || is_pr_channel(&channel.to_string()) {
            // Check the user's auto-install preference
            let should_auto_install = match config_data.settings.auto_install_channels {
                Some(auto_install) => auto_install, // User has explicitly set a preference
                None => {
                    // User hasn't set a preference - prompt in interactive mode, default to false in non-interactive
                    if is_interactive() {
                        handle_auto_install_prompt(channel, paths)?
                    } else {
                        false
                    }
                }
            };

            if should_auto_install {
                // Install the channel using juliaup
                let is_automatic = config_data.settings.auto_install_channels == Some(true);
                spawn_juliaup_add(channel, paths, is_automatic)?;

                // Reload the config to get the newly installed channel
                let updated_config_file = load_config_db(paths, None)
                    .with_context(|| "Failed to reload configuration after installing channel.")?;

                if let Some(channel_info) = updated_config_file.data.installed_channels.get(channel)
                {
                    return get_julia_path_from_installed_channel(
                        versions_db,
                        &updated_config_file.data,
                        channel,
                        juliaupconfig_path,
                        channel_info,
                    );
                } else {
                    return Err(anyhow!(
                        "Channel '{}' was installed but could not be found in configuration.",
                        channel
                    )
                    .into());
                }
            }
            // If we reach here, either installation failed or user declined
        }
    }

    // Original error handling for non-command-line sources or invalid channels
    let error = match juliaup_channel_source {
        JuliaupChannelSource::CmdLine => {
            if channel_valid {
                UserError { msg: format!("`{}` is not installed. Please run `juliaup add {}` to install channel or version.", channel, channel) }
            } else if is_pr_channel(&channel.to_string()) {
                UserError { msg: format!("`{}` is not installed. Please run `juliaup add {}` to install pull request channel if available.", channel, channel) }
            } else {
                UserError { msg: format!("Invalid Juliaup channel `{}`. Please run `juliaup list` to get a list of valid channels and versions.",  channel) }
            }
        },
        JuliaupChannelSource::EnvVar=> {
            if channel_valid {
                UserError { msg: format!("`{}` from environment variable JULIAUP_CHANNEL is not installed. Please run `juliaup add {}` to install channel or version.", channel, channel) }
            } else if is_pr_channel(&channel.to_string()) {
                UserError { msg: format!("`{}` from environment variable JULIAUP_CHANNEL is not installed. Please run `juliaup add {}` to install pull request channel if available.", channel, channel) }
            } else {
                UserError { msg: format!("Invalid Juliaup channel `{}` from environment variable JULIAUP_CHANNEL. Please run `juliaup list` to get a list of valid channels and versions.",  channel) }
            }
        },
        JuliaupChannelSource::Override=> {
            if channel_valid {
                UserError { msg: format!("`{}` from directory override is not installed. Please run `juliaup add {}` to install channel or version.", channel, channel) }
            } else if is_pr_channel(&channel.to_string()){
                UserError { msg: format!("`{}` from directory override is not installed. Please run `juliaup add {}` to install pull request channel if available.", channel, channel) }
            } else {
                UserError { msg: format!("Invalid Juliaup channel `{}` from directory override. Please run `juliaup list` to get a list of valid channels and versions.",  channel) }
            }
        },
        JuliaupChannelSource::Default => UserError {msg: format!("The Juliaup configuration is in an inconsistent state, the currently configured default channel `{}` is not installed.", channel) }
    };

    Err(error.into())
}

fn get_julia_path_from_installed_channel(
    versions_db: &JuliaupVersionDB,
    config_data: &JuliaupConfig,
    channel: &str,
    juliaupconfig_path: &Path,
    channel_info: &JuliaupConfigChannel,
) -> Result<(PathBuf, Vec<String>)> {
    match channel_info {
        JuliaupConfigChannel::LinkedChannel { command, args } => Ok((
            PathBuf::from(command),
            args.as_ref().map_or_else(Vec::new, |v| v.clone()),
        )),
        JuliaupConfigChannel::SystemChannel { version } => {
            let path = &config_data
                .installed_versions.get(version)
                .ok_or_else(|| anyhow!("The juliaup configuration is in an inconsistent state, the channel {} is pointing to Julia version {}, which is not installed.", channel, version))?.path;

            check_channel_uptodate(channel, version, versions_db).with_context(|| {
                format!(
                    "The Julia launcher failed while checking whether the channel {} is up-to-date.",
                    channel
                )
            })?;
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
            Ok((absolute_path.into_path_buf(), Vec::new()))
        }
        JuliaupConfigChannel::DirectDownloadChannel {
            path,
            url: _,
            local_etag,
            server_etag,
            version: _,
        } => {
            if local_etag != server_etag {
                if channel.starts_with("nightly") {
                    // Nightly is updateable several times per day so this message will show
                    // more often than not unless folks update a couple of times a day.
                    // Also, folks using nightly are typically more experienced and need
                    // less detailed prompting
                    eprintln!(
                        "A new `nightly` version is available. Install with `juliaup update`."
                    );
                } else {
                    eprintln!(
                        "A new version of Julia for the `{}` channel is available. Run:",
                        channel
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
            Ok((absolute_path.into_path_buf(), Vec::new()))
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

    // Parse command line
    let mut channel_from_cmd_line: Option<String> = None;
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let first_arg = &args[1];

        if let Some(stripped) = first_arg.strip_prefix('+') {
            channel_from_cmd_line = Some(stripped.to_string());
        }
    }

    let (julia_channel_to_use, juliaup_channel_source) =
        if let Some(channel) = channel_from_cmd_line {
            (channel, JuliaupChannelSource::CmdLine)
        } else if let Ok(channel) = std::env::var("JULIAUP_CHANNEL") {
            (channel, JuliaupChannelSource::EnvVar)
        } else if let Ok(Some(channel)) = get_override_channel(&config_file) {
            (channel, JuliaupChannelSource::Override)
        } else if let Some(channel) = config_file.data.default.clone() {
            (channel, JuliaupChannelSource::Default)
        } else {
            return Err(anyhow!(
                "The Julia launcher failed to figure out which juliaup channel to use."
            ));
        };

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
