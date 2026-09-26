//! Analysis of the command line arguments passed to the `julia` launcher.

use crate::version_selection::julia_option_requires_arg;

/// Whether Julia will start an interactive REPL for the given arguments.
///
/// `args` is the full argument list including the program name, optionally
/// followed by a `+channel` argument. Julia starts a REPL unless a script, a
/// `-e`/`-E` expression or `-` (read from stdin) is given, or unless `-i` forces
/// interactive mode. `--version` and `--help` never start a REPL.
pub fn starts_repl(args: &[String]) -> bool {
    let mut force_interactive = false;
    let mut runs_program = false;

    let mut iter = args.iter().skip(1).peekable();
    if iter.peek().is_some_and(|arg| arg.starts_with('+')) {
        iter.next();
    }

    while let Some(arg) = iter.next() {
        if arg == "--" {
            // Everything after `--` is the script and its arguments
            runs_program = iter.next().is_some();
            break;
        }
        if arg == "-" || !arg.starts_with('-') {
            // Program read from stdin, or a script file
            runs_program = true;
            break;
        }

        let name = arg.split('=').next().unwrap_or(arg);
        match name {
            "-v" | "--version" | "-h" | "--help" | "--help-hidden" => return false,
            "-i" | "--interactive" => force_interactive = true,
            "-e" | "--eval" | "-E" | "--print" => runs_program = true,
            _ => {
                // Short options with an attached expression, e.g. `-e1+1`
                if !arg.starts_with("--") && (arg.starts_with("-e") || arg.starts_with("-E")) {
                    runs_program = true;
                }
            }
        }

        if julia_option_requires_arg(arg) {
            iter.next();
        }
    }

    force_interactive || !runs_program
}

/// Whether Julia only prints information (`--version` or `--help`) for the
/// given arguments, instead of running code.
pub fn prints_info_only(args: &[String]) -> bool {
    let mut iter = args.iter().skip(1).peekable();
    if iter.peek().is_some_and(|arg| arg.starts_with('+')) {
        iter.next();
    }

    while let Some(arg) = iter.next() {
        if arg == "--" || arg == "-" || !arg.starts_with('-') {
            return false;
        }
        if matches!(
            arg.as_str(),
            "-v" | "--version" | "-h" | "--help" | "--help-hidden"
        ) {
            return true;
        }
        if julia_option_requires_arg(arg) {
            iter.next();
        }
    }
    false
}

/// Whether the value of the `CI` environment variable indicates that we are
/// running on a continuous integration system.
pub fn is_ci_value(value: Option<&str>) -> bool {
    match value {
        None => false,
        Some(v) => {
            let v = v.trim();
            !(v.is_empty() || v == "0" || v.eq_ignore_ascii_case("false"))
        }
    }
}

/// Whether we are running on a continuous integration system.
pub fn is_ci() -> bool {
    is_ci_value(std::env::var("CI").ok().as_deref())
}

/// The name of the launcher flag that controls automatic instantiation.
pub const AUTO_INSTANTIATE_FLAG: &str = "--auto-instantiate";

/// The environment variable that controls automatic instantiation.
pub const AUTO_INSTANTIATE_ENV: &str = "JULIA_AUTO_INSTANTIATE";

/// What the launcher installs automatically for the active project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoInstantiate {
    /// Nothing.
    None,
    /// The Julia version recorded in the project's manifest.
    Julia,
    /// The packages recorded in the project's manifest.
    Pkg,
    /// Both the Julia version and the packages.
    All,
}

impl AutoInstantiate {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "none" => Ok(AutoInstantiate::None),
            "julia" => Ok(AutoInstantiate::Julia),
            "pkg" => Ok(AutoInstantiate::Pkg),
            "all" => Ok(AutoInstantiate::All),
            other => Err(format!(
                "Invalid auto-instantiate value `{}`. Valid values are `none`, `julia`, `pkg` and `all`.",
                other
            )),
        }
    }

    /// Whether the Julia version recorded in the manifest is installed automatically.
    pub fn includes_julia(self) -> bool {
        matches!(self, AutoInstantiate::Julia | AutoInstantiate::All)
    }

    /// Whether the packages recorded in the manifest are installed automatically.
    pub fn includes_pkg(self) -> bool {
        matches!(self, AutoInstantiate::Pkg | AutoInstantiate::All)
    }
}

/// Remove `--auto-instantiate[=<level>]` from the Julia options in `args` and
/// return the remaining arguments together with the requested level.
///
/// The flag is only recognized among the options before the first positional
/// argument or `--`, because later arguments belong to the Julia program. A
/// bare `--auto-instantiate` means `all`. If the flag is given more than once,
/// the last one wins.
pub fn extract_auto_instantiate(
    args: &[String],
) -> Result<(Vec<String>, Option<AutoInstantiate>), String> {
    let mut remaining = Vec::with_capacity(args.len());
    let mut level = None;

    let mut iter = args.iter();
    // Program name
    if let Some(program) = iter.next() {
        remaining.push(program.clone());
    }
    let mut iter = iter.peekable();
    if let Some(channel) = iter.next_if(|arg| arg.starts_with('+')) {
        remaining.push(channel.clone());
    }

    while let Some(arg) = iter.next() {
        if arg == "--" || !arg.starts_with('-') || arg == "-" {
            // The rest belongs to the Julia program
            remaining.push(arg.clone());
            remaining.extend(iter.cloned());
            break;
        }

        if arg == AUTO_INSTANTIATE_FLAG {
            level = Some(AutoInstantiate::All);
            continue;
        }
        if let Some(value) = arg
            .strip_prefix(AUTO_INSTANTIATE_FLAG)
            .and_then(|rest| rest.strip_prefix('='))
        {
            level = Some(AutoInstantiate::parse(value)?);
            continue;
        }

        remaining.push(arg.clone());
        if julia_option_requires_arg(arg) {
            // The value of this option, which could look like our flag
            if let Some(value) = iter.next() {
                remaining.push(value.clone());
            }
        }
    }

    Ok((remaining, level))
}

/// The auto-instantiate level from the environment variable, if it is set.
pub fn auto_instantiate_from_env() -> Result<Option<AutoInstantiate>, String> {
    match std::env::var(AUTO_INSTANTIATE_ENV) {
        Ok(value) if !value.trim().is_empty() => {
            AutoInstantiate::parse(&value).map(Some).map_err(|err| {
                format!(
                    "{} (from environment variable {})",
                    err, AUTO_INSTANTIATE_ENV
                )
            })
        }
        _ => Ok(None),
    }
}

/// The environment variable that controls whether the launcher may prompt the
/// user and print informational messages.
pub const LAUNCHER_PROMPTS_ENV: &str = "JULIA_LAUNCHER_PROMPTS";

/// Parse a `JULIA_LAUNCHER_PROMPTS` value. Returns whether prompts and
/// informational messages are allowed; unset or empty means `all`.
pub fn parse_launcher_prompts(value: Option<&str>) -> Result<bool, String> {
    match value.map(str::trim) {
        None | Some("") | Some("all") => Ok(true),
        Some("none") => Ok(false),
        Some(other) => Err(format!(
            "Invalid value `{}` for environment variable {}. Valid values are `all` and `none`.",
            other, LAUNCHER_PROMPTS_ENV
        )),
    }
}

/// Whether the launcher may prompt the user and print informational messages,
/// according to the `JULIA_LAUNCHER_PROMPTS` environment variable.
pub fn launcher_prompts_from_env() -> Result<bool, String> {
    parse_launcher_prompts(std::env::var(LAUNCHER_PROMPTS_ENV).ok().as_deref())
}
