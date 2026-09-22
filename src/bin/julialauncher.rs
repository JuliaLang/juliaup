use anyhow::{anyhow, bail, Context, Result};
use console::{style, Term};
use dialoguer::Select;
use is_terminal::IsTerminal;
use itertools::Itertools;
use juliaup::config_file::{
    load_config_db_lockfree, JuliaupConfig, JuliaupConfigChannel, JuliaupConfigVersion,
};
use juliaup::global_paths::get_paths;
use juliaup::jsonstructs_versionsdb::JuliaupVersionDB;
use juliaup::launcher_args::{
    auto_instantiate_from_env, extract_auto_instantiate, is_ci, starts_repl, AUTO_INSTANTIATE_ENV,
    AUTO_INSTANTIATE_FLAG,
};
use juliaup::operations::{is_pr_channel, is_valid_channel};
use juliaup::project_instantiation::{check_instantiation, depot_paths, InstantiationNeed};
use juliaup::utils::{print_juliaup_style, resolve_julia_binary_path, JuliaupMessageType};
use juliaup::version_selection::{
    determine_project_context, manifest_for_julia_version, parse_db_version, resolve_auto_channel,
    ProjectContext, UnknownJuliaVersion,
};
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
use std::process::Stdio;
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

/// How much we can interact with the user.
#[derive(Debug, Clone, Copy)]
struct Interactivity {
    /// stdin and stderr are terminals and we are not running on CI, so we can
    /// ask the user questions.
    can_prompt: bool,
    /// `can_prompt`, and Julia is about to start an interactive REPL.
    starts_repl: bool,
}

impl Interactivity {
    fn detect(args: &[String]) -> Self {
        let can_prompt =
            std::io::stdin().is_terminal() && std::io::stderr().is_terminal() && !is_ci();
        Interactivity {
            can_prompt,
            starts_repl: can_prompt && starts_repl(args),
        }
    }
}

/// Run `juliaup` with the given arguments and wait for it to finish. Its output
/// goes to stderr, so that the stdout of the launcher stays clean.
fn run_juliaup(args: &[&str]) -> Result<std::process::ExitStatus> {
    let juliaup_path = get_juliaup_path().with_context(|| "Failed to obtain juliaup path.")?;

    std::process::Command::new(juliaup_path)
        .args(args)
        .stdout(std::io::stderr())
        .status()
        .with_context(|| format!("Failed to start `juliaup {}`.", args.join(" ")))
}

fn handle_auto_install_prompt(channel: &str, interactivity: Interactivity) -> Result<bool> {
    if !interactivity.can_prompt {
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
        .interact_opt()?;

    match selection {
        Some(0) => Ok(true),
        Some(1) => {
            set_auto_install_preference()?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn set_auto_install_preference() -> Result<()> {
    let status = run_juliaup(&["config", "autoinstallchannels", "true"])?;
    if !status.success() {
        bail!("Failed to save the auto-install preference to the juliaup configuration.");
    }
    Ok(())
}

fn spawn_juliaup_add(channel: &str, reason: &str) -> Result<()> {
    print_juliaup_style(
        "Installing",
        &format!("Julia {} {}", channel, reason),
        JuliaupMessageType::Progress,
    );

    let status = run_juliaup(&["add", channel])?;

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

/// Refresh the versions db by running juliaup and waiting for it. Returns
/// whether the refresh succeeded.
fn refresh_versions_db() -> Result<bool> {
    let juliaup_path = get_juliaup_path().with_context(|| "Failed to obtain juliaup path.")?;

    let status = std::process::Command::new(juliaup_path)
        .arg("0cf1528f-0b15-46b1-9ac9-e5bf5ccccbcf") // Our internal command to update the versions db
        .stdout(std::io::stderr())
        .stdin(Stdio::null())
        .status()
        .with_context(|| "Failed to start juliaup for version db update.")?;

    Ok(status.success())
}

/// The Julia version selected from the active project's manifest.
#[derive(Debug, Clone)]
struct AutoSelection {
    context: ProjectContext,
    /// The Julia version recorded in the manifest.
    julia_version: String,
}

impl AutoSelection {
    fn manifest_file(&self) -> &Path {
        // An auto selection always comes from a manifest
        self.context
            .manifest_file
            .as_deref()
            .unwrap_or(Path::new("Manifest.toml"))
    }

    /// The manifest file name if it sits next to the project file, otherwise
    /// its full path.
    fn manifest_display(&self) -> String {
        if self.context.manifest_is_elsewhere() {
            display_path(self.manifest_file())
        } else {
            self.manifest_file()
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        }
    }

    /// Description of the Julia version, including the channel if it differs.
    fn version_display(&self, channel: &str) -> String {
        if channel == self.julia_version {
            format!("Julia {}", self.julia_version)
        } else {
            format!("Julia {} (channel `{}`)", self.julia_version, channel)
        }
    }
}

fn display_path(path: &Path) -> String {
    dunce::simplified(path).display().to_string()
}

fn project_detection_error(err: anyhow::Error) -> UserError {
    UserError {
        msg: format!(
            "Failed to determine the Julia version for the active project.\n  {:#}\nTo start Julia regardless, select a channel explicitly, e.g. `julia +release`.",
            err
        ),
    }
}

/// Determine the Juliaup channel from the Julia version recorded in the
/// active project's manifest.
fn get_auto_channel(
    args: &[String],
    versions_db: &mut JuliaupVersionDB,
    paths: &juliaup::global_paths::GlobalPaths,
) -> Result<Option<(String, AutoSelection)>> {
    let context = determine_project_context(args).map_err(project_detection_error)?;

    let Some(context) = context else {
        return Ok(None);
    };
    let Some(julia_version) = context.julia_version.clone() else {
        return Ok(None);
    };
    let selection = AutoSelection {
        context,
        julia_version,
    };

    let channel = match resolve_auto_channel(&selection.julia_version, versions_db) {
        Ok(channel) => channel,
        Err(err) if err.downcast_ref::<UnknownJuliaVersion>().is_some() => {
            // The versions db might be outdated, so refresh it and try again
            print_juliaup_style(
                "Info",
                &format!(
                    "Julia {} (recorded in {}) is not in the local list of Julia versions, refreshing it.",
                    selection.julia_version,
                    display_path(selection.manifest_file())
                ),
                JuliaupMessageType::Progress,
            );
            let refreshed = refresh_versions_db()?;
            *versions_db = load_versions_db(paths)
                .with_context(|| "The Julia launcher failed to load a versions db.")?;

            match resolve_auto_channel(&selection.julia_version, versions_db) {
                Ok(channel) => channel,
                Err(err) if err.downcast_ref::<UnknownJuliaVersion>().is_some() => {
                    return Err(UserError {
                        msg: format!(
                            "Julia {} recorded in `{}` is not a known Julia release{}.\nTo start Julia regardless, select a channel explicitly, e.g. `julia +release`.",
                            selection.julia_version,
                            display_path(selection.manifest_file()),
                            if refreshed {
                                ""
                            } else {
                                ", and refreshing the list of Julia versions failed"
                            }
                        ),
                    }
                    .into());
                }
                Err(err) => return Err(err),
            }
        }
        Err(err) => return Err(err),
    };

    Ok(Some((channel, selection)))
}

/// What to do when the Julia version required by the project is not installed.
enum MissingVersionChoice {
    Install,
    InstallAndRemember,
    Cancel,
}

fn prompt_install_required_version(
    selection: &AutoSelection,
    channel: &str,
) -> Result<MissingVersionChoice> {
    let choice = Select::new()
        .with_prompt(format!(
            "{} Project {} requires {} (from {}), which is not installed.",
            style("Question:").yellow().bold(),
            display_path(selection.context.project_dir()),
            selection.version_display(channel),
            selection.manifest_display()
        ))
        .item(format!(
            "Install {} and start it",
            selection.version_display(channel)
        ))
        .item("Install, and always install required Julia versions automatically")
        .item("Cancel")
        .default(0)
        .interact_opt()?;

    Ok(match choice {
        Some(0) => MissingVersionChoice::Install,
        Some(1) => MissingVersionChoice::InstallAndRemember,
        _ => MissingVersionChoice::Cancel,
    })
}

fn missing_required_version_error(selection: &AutoSelection, channel: &str) -> UserError {
    let mut msg = format!(
        "This project requires {}, which is not installed.\n  Project:  {}\n  Manifest: {} (julia_version = \"{}\")\n\nFix it with one of:\n",
        selection.version_display(channel),
        display_path(&selection.context.project_file),
        display_path(selection.manifest_file()),
        selection.julia_version,
    );
    msg.push_str(&format!(
        "  {:<44}install this version\n",
        format!("juliaup add {}", channel)
    ));
    msg.push_str(&format!(
        "  {:<44}install automatically on launch\n  {:<44}(or set {}=julia)\n",
        format!("julia {}=julia ...", AUTO_INSTANTIATE_FLAG),
        "",
        AUTO_INSTANTIATE_ENV
    ));
    msg.push_str(&format!(
        "  {:<44}always install required versions automatically",
        "juliaup config autoinstallchannels true"
    ));
    UserError { msg }
}

/// The Julia version a channel provides, if known.
fn installed_channel_version(config_data: &JuliaupConfig, channel: &str) -> Option<Version> {
    let channel = match config_data.installed_channels.get(channel)? {
        JuliaupConfigChannel::AliasChannel { target, .. } => {
            config_data.installed_channels.get(target)?
        }
        other => other,
    };
    match channel {
        JuliaupConfigChannel::SystemChannel { version }
        | JuliaupConfigChannel::DirectDownloadChannel { version, .. } => {
            parse_db_version(version).ok()
        }
        _ => None,
    }
}

/// Run `Pkg.instantiate` for the active project if any of the packages recorded
/// in its manifest are not installed.
fn instantiate_project_if_needed(
    args: &[String],
    auto_selection: Option<&AutoSelection>,
    julia_path: &Path,
    julia_args: &[String],
    julia_version: Option<Version>,
) -> Result<()> {
    let context = match auto_selection {
        Some(selection) => Some(selection.context.clone()),
        None => determine_project_context(args).map_err(project_detection_error)?,
    };
    let Some(context) = context else {
        return Ok(());
    };

    // The manifest that the Julia we are about to launch will load
    let manifest_file = match auto_selection {
        Some(_) => context.manifest_file.clone(),
        None => manifest_for_julia_version(
            &context.project_file,
            julia_version.map(|v| (v.major, v.minor)),
        )
        .map_err(project_detection_error)?
        .map(|m| m.path),
    };

    let depots = depot_paths(
        std::env::var_os("JULIA_DEPOT_PATH").as_deref(),
        Some(julia_path),
    );
    let need = check_instantiation(manifest_file.as_deref(), context.has_deps, &depots)
        .map_err(project_detection_error)?;
    let Some(need) = need else {
        return Ok(());
    };

    let reason = match need {
        InstantiationNeed::NoManifest => "no manifest yet".to_string(),
        InstantiationNeed::MissingPackages(packages) => {
            let mut names = packages
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            if packages.len() > 5 {
                names.push_str(", ...");
            }
            format!(
                "{} package{} not installed: {}",
                packages.len(),
                if packages.len() == 1 { "" } else { "s" },
                names
            )
        }
    };
    print_juliaup_style(
        "Instantiating",
        &format!(
            "project {} ({})",
            display_path(context.project_dir()),
            reason
        ),
        JuliaupMessageType::Progress,
    );

    // Run the resolved Julia binary directly, not through the launcher. Its
    // output goes to stderr so that the stdout of the launcher stays clean.
    let status = std::process::Command::new(julia_path)
        .args(julia_args)
        .arg(format!("--project={}", context.project_dir().display()))
        .args([
            "--startup-file=no",
            "--history-file=no",
            "-e",
            "import Pkg; Pkg.instantiate()",
        ])
        .stdin(Stdio::null())
        .stdout(std::io::stderr())
        .status()
        .with_context(|| "Failed to start Julia to instantiate the project.")?;

    if !status.success() {
        return Err(UserError {
            msg: format!(
                "Failed to instantiate the project {} (Pkg.instantiate exited with code {:?}).",
                display_path(context.project_dir()),
                status.code()
            ),
        }
        .into());
    }

    Ok(())
}

fn check_channel_uptodate(
    channel: &str,
    current_version: &str,
    versions_db: &JuliaupVersionDB,
) -> Result<()> {
    let Some(channel_info) = versions_db.available_channels.get(channel) else {
        // Channel not in versions DB (e.g. from a custom depot), skip the update check
        return Ok(());
    };
    let latest_version = &channel_info.version;

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

/// The Julia binary to launch.
struct ResolvedJulia {
    path: PathBuf,
    args: Vec<String>,
}

/// Options that influence how the launcher resolves a channel to a Julia binary.
struct ChannelResolveOptions<'a> {
    interactivity: Interactivity,
    /// The project-based selection, when the channel comes from a manifest.
    auto_selection: Option<&'a AutoSelection>,
    /// Whether `--auto-instantiate`/`JULIA_AUTO_INSTANTIATE` asks for the Julia
    /// version required by the project to be installed automatically. `None`
    /// if neither was specified.
    auto_install_julia: Option<bool>,
}

fn get_julia_path_from_channel(
    versions_db: &JuliaupVersionDB,
    config_data: &JuliaupConfig,
    channel: &str,
    juliaupconfig_path: &Path,
    juliaup_channel_source: JuliaupChannelSource,
    paths: &juliaup::global_paths::GlobalPaths,
    options: &ChannelResolveOptions,
) -> Result<ResolvedJulia> {
    // First check if the channel is an alias and extract its args
    let (resolved_channel, alias_args) = match config_data.installed_channels.get(channel) {
        Some(JuliaupConfigChannel::AliasChannel { target, args }) => {
            (target.to_string(), args.clone().unwrap_or_default())
        }
        _ => (channel.to_string(), Vec::new()),
    };

    let channel_valid = is_valid_channel(versions_db, &resolved_channel)?;
    let show_update_notices = options.interactivity.starts_repl;

    // First check if the channel is already installed
    if let Some(channel_info) = config_data.installed_channels.get(&resolved_channel) {
        let (path, args) = get_julia_path_from_installed_channel(
            versions_db,
            config_data,
            &resolved_channel,
            juliaupconfig_path,
            channel_info,
            alias_args.clone(),
            show_update_notices,
        )?;
        return Ok(ResolvedJulia { path, args });
    }

    // For auto-resolved channels (from manifest), check if the Julia version
    // that the channel maps to is already installed via another channel.
    // This avoids prompting the user to install e.g. channel "1.12.5" when
    // the "release" channel already provides Julia 1.12.5.
    if matches!(juliaup_channel_source, JuliaupChannelSource::Auto) {
        if let Some(version_info) = versions_db
            .available_channels
            .get(&resolved_channel)
            .and_then(|ch| config_data.installed_versions.get(&ch.version))
        {
            let path = resolve_version_path(version_info, juliaupconfig_path)?;
            return Ok(ResolvedJulia {
                path,
                args: alias_args,
            });
        }
    }

    let auto_selection = match juliaup_channel_source {
        JuliaupChannelSource::Auto => options.auto_selection,
        _ => None,
    };

    // Handle auto-installation for command line channel selection and auto-resolved channels
    if matches!(
        juliaup_channel_source,
        JuliaupChannelSource::CmdLine | JuliaupChannelSource::Auto
    ) && (channel_valid
        || is_pr_channel(&resolved_channel)
        || is_nightly_channel(&resolved_channel))
    {
        let install_reason = if let Some(selection) = auto_selection {
            // The channel is required by the active project
            match (
                options.auto_install_julia,
                config_data.settings.auto_install_channels,
            ) {
                (Some(true), _) => Some("required by the project (auto-instantiate)"),
                (None, Some(true)) => Some("automatically per juliaup settings"),
                (None, Some(false)) => None,
                _ if options.interactivity.can_prompt => {
                    match prompt_install_required_version(selection, &resolved_channel)? {
                        MissingVersionChoice::Install => Some("as requested"),
                        MissingVersionChoice::InstallAndRemember => {
                            set_auto_install_preference()?;
                            Some("as requested")
                        }
                        MissingVersionChoice::Cancel => None,
                    }
                }
                _ => None,
            }
        } else {
            // Check the user's auto-install preference
            match config_data.settings.auto_install_channels {
                Some(true) => Some("automatically per juliaup settings"),
                Some(false) => None,
                // User hasn't set a preference - prompt in interactive mode, default to false in non-interactive
                None => {
                    if handle_auto_install_prompt(&resolved_channel, options.interactivity)? {
                        Some("as requested")
                    } else {
                        None
                    }
                }
            }
        };

        if let Some(reason) = install_reason {
            // Install the channel using juliaup
            spawn_juliaup_add(&resolved_channel, reason)?;

            // Reload the config to get the newly installed channel
            let updated_config_file = load_config_db_lockfree(paths)
                .with_context(|| "Failed to reload configuration after installing channel.")?;

            let updated_channel_info = updated_config_file
                .data
                .installed_channels
                .get(&resolved_channel);

            if let Some(channel_info) = updated_channel_info {
                let (path, args) = get_julia_path_from_installed_channel(
                    versions_db,
                    &updated_config_file.data,
                    &resolved_channel,
                    juliaupconfig_path,
                    channel_info,
                    alias_args,
                    false,
                )?;
                return Ok(ResolvedJulia { path, args });
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
        JuliaupChannelSource::Auto => match auto_selection {
            Some(selection) if channel_valid || is_pr_channel(&resolved_channel) || is_nightly_channel(&resolved_channel) => {
                missing_required_version_error(selection, &resolved_channel)
            }
            _ => UserError { msg: format!("Invalid Juliaup channel `{resolved_channel}` resolved from project manifest. Please run `juliaup list` to get a list of valid channels and versions.") },
        },
        JuliaupChannelSource::Default => UserError {msg: format!("The Juliaup configuration is in an inconsistent state, the currently configured default channel `{resolved_channel}` is not installed.") }
    };

    Err(error.into())
}

fn resolve_version_path(
    version_info: &JuliaupConfigVersion,
    juliaupconfig_path: &Path,
) -> Result<PathBuf> {
    let config_dir = juliaupconfig_path.parent().unwrap(); // unwrap OK because there should always be a parent

    // Use pre-computed binary_path if available (new installations),
    // otherwise fall back to runtime resolution (backward compatibility)
    let absolute_path = if let Some(ref binary_path) = version_info.binary_path {
        config_dir.join(binary_path)
    } else {
        let base_path = config_dir.join(&version_info.path);
        resolve_julia_binary_path(&base_path)?
    }
    .normalize()
    .with_context(|| {
        format!(
            "Failed to normalize path for Julia binary, starting from `{}`.",
            juliaupconfig_path.display()
        )
    })?;

    Ok(absolute_path.into_path_buf())
}

fn get_julia_path_from_installed_channel(
    versions_db: &JuliaupVersionDB,
    config_data: &JuliaupConfig,
    channel: &str,
    juliaupconfig_path: &Path,
    channel_info: &JuliaupConfigChannel,
    alias_args: Vec<String>,
    show_update_notices: bool,
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
            let version_info = config_data
                .installed_versions.get(version)
                .ok_or_else(|| anyhow!("The juliaup configuration is in an inconsistent state, the channel {channel} is pointing to Julia version {version}, which is not installed."))?;

            if show_update_notices {
                check_channel_uptodate(channel, version, versions_db).with_context(|| {
                    format!("The Julia launcher failed while checking whether the channel {channel} is up-to-date.")
                })?;
            }

            let path = resolve_version_path(version_info, juliaupconfig_path)?;

            Ok((path, alias_args))
        }
        JuliaupConfigChannel::DirectDownloadChannel {
            path,
            url: _,
            local_etag,
            server_etag,
            version: _,
            binary_path,
        } => {
            if local_etag != server_etag && show_update_notices {
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

            let config_dir = juliaupconfig_path.parent().unwrap();

            // Use pre-computed binary_path if available (new installations),
            // otherwise fall back to runtime resolution (backward compatibility)
            let absolute_path = if let Some(ref bp) = binary_path {
                config_dir.join(bp)
            } else {
                let base_path = config_dir.join(path);
                resolve_julia_binary_path(&base_path)?
            }
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

fn run_app() -> Result<i32> {
    if std::io::stdout().is_terminal() {
        // Set console title
        let term = Term::stdout();
        term.set_title("Julia");
    }

    let paths = get_paths().with_context(|| "Trying to load all global paths.")?;

    do_initial_setup(&paths.juliaupconfig)
        .with_context(|| "The Julia launcher failed to run the initial setup steps.")?;

    // Read the configuration without taking the configuration lock, so that
    // launching Julia can never block on (or be stalled by) the lock. This is
    // safe because all config writers replace the file atomically.
    let config_file = load_config_db_lockfree(&paths)
        .with_context(|| "The Julia launcher failed to load a configuration file.")?;

    let mut versiondb_data = load_versions_db(&paths)
        .with_context(|| "The Julia launcher failed to load a versions db.")?;

    // Parse command line
    let mut channel_from_cmd_line: Option<String> = None;
    let raw_args: Vec<String> = std::env::args().collect();
    let (args, flag_auto_instantiate) =
        extract_auto_instantiate(&raw_args).map_err(|msg| UserError { msg })?;
    let auto_instantiate = match flag_auto_instantiate {
        Some(level) => Some(level),
        None => auto_instantiate_from_env().map_err(|msg| UserError { msg })?,
    };
    if args.len() > 1 {
        let first_arg = &args[1];

        if let Some(stripped) = first_arg.strip_prefix('+') {
            channel_from_cmd_line = Some(stripped.to_string());
        }
    }

    let interactivity = Interactivity::detect(&args);

    let mut auto_selection: Option<AutoSelection> = None;

    let (julia_channel_to_use, juliaup_channel_source) = if let Some(channel) =
        channel_from_cmd_line
    {
        (channel, JuliaupChannelSource::CmdLine)
    } else if let Ok(channel) = std::env::var("JULIAUP_CHANNEL") {
        (channel, JuliaupChannelSource::EnvVar)
    } else if let Ok(Some(channel)) = get_override_channel(&config_file) {
        (channel, JuliaupChannelSource::Override)
    } else if let Some((channel, selection)) = if config_file.data.settings.manifest_version_detect
        || auto_instantiate.is_some_and(|level| level.includes_julia())
    {
        get_auto_channel(&args, &mut versiondb_data, &paths)?
    } else {
        None
    } {
        auto_selection = Some(selection);
        (channel, JuliaupChannelSource::Auto)
    } else if let Some(channel) = config_file.data.default.clone() {
        (channel, JuliaupChannelSource::Default)
    } else {
        return Err(anyhow!(
            "The Julia launcher failed to figure out which juliaup channel to use."
        ));
    };

    let resolve_options = ChannelResolveOptions {
        interactivity,
        auto_selection: auto_selection.as_ref(),
        auto_install_julia: auto_instantiate.map(|level| level.includes_julia()),
    };

    let resolved_julia = get_julia_path_from_channel(
        &versiondb_data,
        &config_file.data,
        &julia_channel_to_use,
        &paths.juliaupconfig,
        juliaup_channel_source,
        &paths,
        &resolve_options,
    )
    .with_context(|| {
        format!(
            "The Julia launcher failed to determine the command for the `{}` channel.",
            julia_channel_to_use
        )
    })?;

    let julia_path = resolved_julia.path;

    if auto_instantiate.is_some_and(|level| level.includes_pkg()) {
        let julia_version = match &auto_selection {
            Some(selection) => Version::parse(&selection.julia_version).ok(),
            None => installed_channel_version(&config_file.data, &julia_channel_to_use),
        };
        instantiate_project_if_needed(
            &args,
            auto_selection.as_ref(),
            &julia_path,
            &resolved_julia.args,
            julia_version,
        )?;
    }

    let mut new_args: Vec<String> = Vec::new();

    for i in resolved_julia.args {
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
                Ok(ForkResult::Parent { .. }) => {
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
