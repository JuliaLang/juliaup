use anyhow::{anyhow, Context, Result};
use console::Term;
use itertools::Itertools;
use juliaup::config_file::{load_config_db, JuliaupConfig, JuliaupConfigChannel};
use juliaup::global_paths::get_paths;
use juliaup::jsonstructs_versionsdb::JuliaupVersionDB;
use juliaup::versions_file::load_versions_db;
use normpath::PathExt;
use std::path::Path;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
#[error("{msg}")]
pub struct UserError {
    msg: String,
}

fn get_juliaup_path() -> Result<PathBuf> {
    let my_own_path = std::env::current_exe()
        .with_context(|| "std::env::current_exe() did not find its own path.")?;

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
            "to install Julia {} and update the `{}` channel to that version.",
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
) -> Result<(PathBuf, Vec<String>)> {
    let channel_info = config_data
            .installed_channels
            .get(channel)
            .ok_or_else(|| match juliaup_channel_source {
                JuliaupChannelSource::CmdLine => UserError { msg: format!("ERROR: Invalid Juliaup channel `{}` at command line.", channel).to_string() }.into(),
                JuliaupChannelSource::EnvVar => UserError { msg: format!("ERROR: Invalid Juliaup channel `{}` in environment variable JULIAUP_CHANNEL.", channel).to_string() }.into(),
                JuliaupChannelSource::Override => UserError { msg: format!("ERROR: Invalid Juliaup channel `{}` in directory override.", channel).to_string() }.into(),
                JuliaupChannelSource::Default => anyhow!("The Juliaup configuration is in an inconsistent state, the currently configured default channel `{}` is not installed.", channel)
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

            if config_data.settings.should_check_channel_update {
                check_channel_uptodate(channel, version, versions_db).with_context(|| {
                    format!(
                        "The Julia launcher failed while checking whether the channe {} is up-to-date.",
                        channel
                    )
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
            return Ok((absolute_path.into_path_buf(), Vec::new()));
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

fn run_app() -> Result<i32> {
    // Set console title
    let term = Term::stdout();
    term.set_title("Julia");

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
        if i > 1 || !v.starts_with('+') {
            new_args.push(v.clone());
        }
    }

    // We set a Ctrl-C handler here that just doesn't do anything, as we want the Julia child
    // process to handle things.
    ctrlc::set_handler(|| ()).with_context(|| "Failed to set the Ctrl-C handler.")?;

    let mut child_process = std::process::Command::new(julia_path)
        .args(&new_args)
        .spawn()
        .with_context(|| "The Julia launcher failed to start Julia.")?; // TODO Maybe include the command we actually tried to start?

    run_versiondb_update(&config_file).with_context(|| "Failed to run version db update")?;

    run_selfupdate(&config_file).with_context(|| "Failed to run selfupdate.")?;

    let status = child_process
        .wait()
        .with_context(|| "Failed to wait for Julia process to finish.")?;

    let code = match status.code() {
        Some(code) => code,
        None => {
            #[cfg(not(windows))]
            {
                use std::os::unix::process::ExitStatusExt;

                let signal = status.signal();

                if let Some(signal) = signal {
                    let signal = nix::sys::signal::Signal::try_from(signal)
                        .with_context(|| format!("Unknown signal value {}.", signal))?;

                    nix::sys::signal::raise(signal).with_context(|| "Failed to raise signal.")?;

                    // We raise the signal twice because SIGSEGV and SIGBUS require that
                    // https://github.com/JuliaLang/juliaup/pull/525#issuecomment-1353526900
                    // https://github.com/rust-lang/rust/blob/984eab57f708e62c09b3d708033fe620130b5f39/library/std/src/sys/unix/stack_overflow.rs#L60-L80
                    nix::sys::signal::raise(signal).with_context(|| "Failed to raise signal.")?;

                    anyhow::bail!("Maybe we should never reach this?");
                } else {
                    anyhow::bail!("We weren't able to extract a signal, this should never happen.");
                }
            }

            #[cfg(windows)]
            {
                anyhow::bail!("There is no exit code, that should not be possible on Windows.");
            }
        }
    };

    Ok(code)
}

fn main() -> Result<std::process::ExitCode> {
    human_panic::setup_panic!(human_panic::Metadata {
        name: "Juliaup launcher".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        authors: "".into(),
        homepage: "https://github.com/JuliaLang/juliaup".into(),
    });

    let env = env_logger::Env::new()
        .filter("JULIAUP_LOG")
        .write_style("JULIAUP_LOG_STYLE");
    env_logger::init_from_env(env);

    let client_status = run_app();

    if let Err(err) = &client_status {
        if let Some(e) = err.downcast_ref::<UserError>() {
            eprintln!("{}", e.msg);

            return Ok(std::process::ExitCode::FAILURE);
        }
    }

    let client_status: u8 = client_status?.try_into().unwrap();

    Ok(std::process::ExitCode::from(client_status))
}
