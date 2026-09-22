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
