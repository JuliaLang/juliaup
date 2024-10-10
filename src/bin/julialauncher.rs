use anyhow::{anyhow, Context, Result};
use console::Term;
use is_terminal::IsTerminal;
use itertools::Itertools;
use juliaup::config_file::{load_config_db, JuliaupConfig, JuliaupConfigChannel};
use juliaup::global_paths::get_paths;
use juliaup::jsonstructs_versionsdb::JuliaupVersionDB;
use juliaup::versions_file::load_versions_db;
#[cfg(not(windows))]
use nix::{
    sys::wait::{waitpid, WaitStatus},
    unistd::{fork, ForkResult},
};
use normpath::PathExt;
use semver::Version;
#[cfg(not(windows))]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, RawHandle};
use std::path::Path;
use std::path::PathBuf;
use toml::Value;
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

            if Utc::now() >= update_time {
                true
            } else {
                false
            }
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

fn check_channel_uptodate(
    channel: &str,
    current_version: &str,
    versions_db: &JuliaupVersionDB,
) -> Result<()> {
    let latest_version = &versions_db
        .available_channels
        .get(channel)
        .ok_or_else(|| {
            anyhow!(
                "The channel `{}` does not exist in the versions database.",
                channel
            )
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

#[derive(PartialEq, Eq)]
enum JuliaupChannelSource {
    CmdLine { channel: String },
    EnvVar { channel: String },
    Override { channel: String },
    Manifest { version: String },
    Default { channel: String },
}

fn get_julia_path_from_channel(
    versions_db: &JuliaupVersionDB,
    config_data: &JuliaupConfig,
    launch_parameters: &JuliaupChannelSource,
    juliaupconfig_path: &Path,
) -> Result<(PathBuf, Vec<String>)> {
    if let JuliaupChannelSource::Manifest { version } = launch_parameters {
        let version_string = versions_db.available_channels.get(version)
            .ok_or_else(|| anyhow!("The project you are trying to launch uses Julia {}, but no such Julia version exists. Please make sure you are using a valid Julia manifest file.", version))?;

        let version_config = config_data.installed_versions.get(&version_string.version)
            .ok_or_else(|| anyhow!("The project you are trying to launch uses Julia {}, but you do not have that version installed. You can install it by running `juliaup add {}`.", version, version))?;

        let absolute_path = juliaupconfig_path
            .parent()
            .unwrap() // unwrap OK because there should always be a parent
            .join(&version_config.path)
            .join("bin")
            .join(format!("julia{}", std::env::consts::EXE_SUFFIX))
            .normalize()
            .with_context(|| {
                format!(
                    "Failed to normalize path for Julia binary, starting from `{}`.",
                    juliaupconfig_path.display()
                )
            })?;

        return Ok((absolute_path.into_path_buf(), Vec::new()));
    } else {
        let channel = match launch_parameters {
            JuliaupChannelSource::CmdLine { channel } => channel,
            JuliaupChannelSource::Default { channel } => channel,
            JuliaupChannelSource::EnvVar { channel } => channel,
            JuliaupChannelSource::Override { channel } => channel,
            _ => unreachable!(),
        };

        let channel_info = config_data
            .installed_channels
            .get(channel)
            .ok_or_else(|| match launch_parameters {
                JuliaupChannelSource::CmdLine {..} => {
                    if versions_db.available_channels.contains_key(channel) {
                        UserError { msg: format!("`{}` is not installed. Please run `juliaup add {}` to install channel or version.", channel, channel) }
                    } else {
                        UserError { msg: format!("ERROR: Invalid Juliaup channel `{}`. Please run `juliaup list` to get a list of valid channels and versions.",  channel) }
                    }
                }.into(),
                JuliaupChannelSource::EnvVar {..} => {
                    if versions_db.available_channels.contains_key(channel) {
                        UserError { msg: format!("`{}` for environment variable JULIAUP_CHANNEL is not installed. Please run `juliaup add {}` to install channel or version.", channel, channel) }
                    } else {
                        UserError { msg: format!("ERROR: Invalid Juliaup channel `{}` in environment variable JULIAUP_CHANNEL. Please run `juliaup list` to get a list of valid channels and versions.",  channel) }
                    }
                }.into(),
                JuliaupChannelSource::Override {..} => {
                    if versions_db.available_channels.contains_key(channel) {
                        UserError { msg: format!("`{}` for directory override is not installed. Please run `juliaup add {}` to install channel or version.", channel, channel) }
                    } else {
                        UserError { msg: format!("ERROR: Invalid Juliaup channel `{}` in directory override. Please run `juliaup list` to get a list of valid channels and versions.",  channel) }
                    }
                }.into(),
                JuliaupChannelSource::Manifest {..} => unreachable!(),
                JuliaupChannelSource::Default {..} => anyhow!("The Juliaup configuration is in an inconsistent state, the currently configured default channel `{}` is not installed.", channel)
            })?;

        match channel_info {
            JuliaupConfigChannel::LinkedChannel { command, args } => {
                return Ok((
                    PathBuf::from(command),
                    args.as_ref().map_or_else(Vec::new, |v| v.clone()),
                ))
            }
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
                return Ok((absolute_path.into_path_buf(), Vec::new()));
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
                return Ok((absolute_path.into_path_buf(), Vec::new()));
            }
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
        .last();

    match juliaup_override {
        Some(val) => Ok(Some(val.channel.clone())),
        None => Ok(None),
    }
}

fn get_program_file(args: &Vec<String>) -> Option<(usize, &String)> {
    let mut program_file: Option<(usize, &String)> = None;
    let no_arg_short_switches = ['v', 'h', 'i', 'q'];
    let no_arg_long_switches = [
        "--version",
        "--help",
        "--help-hidden",
        "--interactive",
        "--quiet",
        // Hidden options
        "--lisp",
        "--image-codegen",
        "--rr-detach",
        "--strip-metadata",
        "--strip-ir",
        "--permalloc-pkgimg",
        "--heap-size-hint",
        "--trim",
    ];
    let mut skip_next = false;
    for (i, arg) in args.iter().skip(1).enumerate() {
        if skip_next {
            skip_next = false;
        } else if arg == "--" {
            if i + 1 < args.len() {
                program_file = Some((i + 1, args.get(i + 1).unwrap()));
            }
            break;
        } else if arg.starts_with("--") {
            if !no_arg_long_switches.contains(&arg.as_str()) && !arg.contains('=') {
                skip_next = true;
            }
        } else if arg.starts_with("-") {
            let arg: Vec<char> = arg.chars().skip(1).collect();
            if arg.iter().all(|&c| no_arg_short_switches.contains(&c)) {
                continue;
            }
            for (j, &c) in arg.iter().enumerate() {
                if no_arg_short_switches.contains(&c) {
                    continue;
                } else if j < arg.len() - 1 {
                    break;
                } else {
                    // `j == arg.len() - 1`
                    skip_next = true;
                }
            }
        } else {
            program_file = Some((i, arg));
            break;
        }
    }
    return program_file;
}

fn get_project(args: &Vec<String>, config: &JuliaupConfig) -> Option<PathBuf> {
    if !config.settings.feature_manifest_support {
        return None
    }

    let program_file = get_program_file(args);
    let recognised_proj_flags: [&str; 4] = ["--project", "--projec", "--proje", "--proj"];
    let mut project_arg: Option<String> = None;
    for arg in args
        .iter()
        .take(program_file.map_or(args.len(), |(i, _)| i))
    {
        if arg.starts_with("--proj") {
            let mut parts = arg.splitn(2, '=');
            if recognised_proj_flags.contains(&parts.next().unwrap_or("")) {
                project_arg = Some(parts.next().unwrap_or("@").to_string());
            }
        }
    }
    let project = if project_arg.is_some() {
        project_arg.unwrap()
    } else if let Ok(val) = std::env::var("JULIA_PROJECT") {
        val
    } else {
        return None;
    };
    if project == "@" {
        return None;
    } else if project == "@." || project == "" {
        let mut path = PathBuf::from(std::env::current_dir().unwrap());
        while !path.join("Project.toml").exists() && !path.join("JuliaProject.toml").exists() {
            if !path.pop() {
                return None;
            }
        }
        return Some(path);
    } else if project == "@script" {
        if let Some((_, file)) = program_file {
            let mut path = PathBuf::from(file);
            path.pop();
            while !path.join("Project.toml").exists() && !path.join("JuliaProject.toml").exists() {
                if !path.pop() {
                    return None;
                }
            }
            return Some(path);
        } else {
            return None;
        }
    } else if project.starts_with('@') {
        let depot = match std::env::var("JULIA_DEPOT_PATH") {
            Ok(val) => match val.split(':').next() {
                Some(p) => PathBuf::from(p),
                None => dirs::home_dir().unwrap().join(".julia"),
            },
            _ => dirs::home_dir().unwrap().join(".julia"),
        };
        let path = depot.join("environments").join(&project[1..]);
        if path.exists() {
            return Some(path);
        } else {
            return None;
        }
    } else {
        return Some(PathBuf::from(project));
    }
}

fn julia_version_from_manifest(path: PathBuf) -> Option<Version> {
    let manifest = if path.join("JuliaManifest.toml").exists() {
        path.join("JuliaManifest.toml")
    } else if path.join("Manifest.toml").exists() {
        path.join("Manifest.toml")
    } else {
        return None;
    };
    let content = std::fs::read_to_string(manifest)
        .ok()?
        .parse::<Value>()
        .ok()?;
    if let Some(manifest_format) = content.get("manifest_format") {
        if manifest_format.as_str()?.starts_with("2.") {
            if let Some(julia_version) = content.get("julia_version") {
                return julia_version.as_str().and_then(|v| Version::parse(v).ok());
            }
        }
    }
    return None;
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

    let config_file = load_config_db(&paths)
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

    let julia_launch_config = if let Some(channel) = channel_from_cmd_line {
        JuliaupChannelSource::CmdLine { channel: channel }
    } else if let Ok(channel) = std::env::var("JULIAUP_CHANNEL") {
        JuliaupChannelSource::EnvVar { channel: channel }
    } else if let Ok(Some(channel)) = get_override_channel(&config_file) {
        JuliaupChannelSource::Override { channel: channel }
    } else if let Some(version) = get_project(&args, &config_file.data).and_then(julia_version_from_manifest) {
        JuliaupChannelSource::Manifest {
            version: version.to_string(),
        }
    } else if let Some(channel) = config_file.data.default.clone() {
        JuliaupChannelSource::Default { channel: channel }
    } else {
        return Err(anyhow!(
            "The Julia launcher failed to figure out which juliaup channel to use."
        ));
    };

    let (julia_path, julia_args) = get_julia_path_from_channel(
        &versiondb_data,
        &config_file.data,
        &julia_launch_config,
        &paths.juliaupconfig,
    )
    .with_context(|| "The Julia launcher failed to determine the Julia version to launch.")?;

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
            std::process::Command::new(&julia_path)
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
                eprintln!("{}", e.msg);

                return Ok(std::process::ExitCode::FAILURE);
            } else {
                return Err(client_status.unwrap_err());
            }
        }
    }

    // TODO https://github.com/rust-lang/rust/issues/111688 is finalized, we should use that instead of calling exit
    std::process::exit(client_status?);
}
