use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use juliaup::cli::CompletionShell;

// IMPORTANT: This CLI wrapper for Julia's Pkg does NOT include the following REPL-only commands:
//
// 1. `activate` - This command changes the active environment within a REPL session.
//    In a CLI context, each invocation is stateless. Users should use Julia's --project
//    flag or JULIA_PROJECT environment variable to specify the project instead.
//
// 2. `undo` - This command undoes the last change within a REPL session.
//    In a CLI context, there is no session state to undo. Each command invocation
//    is independent and stateless.
//
// 3. `redo` - This command redoes an undone change within a REPL session.
//    In a CLI context, there is no session state to redo. Each command invocation
//    is independent and stateless.
//
// These commands are fundamentally REPL-specific as they rely on persistent session state
// that doesn't exist in one-time CLI invocations. DO NOT add these commands to this CLI.

#[derive(Parser)]
#[command(name = "jlpkg")]
#[command(about = "Julia package manager", long_about = None)]
#[command(allow_external_subcommands = true)]
#[command(override_usage = "jlpkg [OPTIONS] [COMMAND]
       jlpkg [OPTIONS] [COMMAND] [ARGS]...

    Julia options can be passed before the command:
        +<channel>         Select Julia channel (e.g., +1.10, +release)
        --project[=<path>] Set project directory
        [...]              Other Julia flags are also supported")]
struct Cli {
    #[command(subcommand)]
    command: Option<PkgCommand>,
}

#[derive(Clone, ValueEnum)]
enum PreserveLevel {
    Installed,
    All,
    Direct,
    Semver,
    None,
    TieredInstalled,
    Tiered,
}

#[derive(Clone, ValueEnum)]
enum UpdatePreserveLevel {
    All,
    Direct,
    None,
}

#[derive(Subcommand)]
enum PkgCommand {
    /// Add packages to project
    Add {
        /// Package specifications to add
        packages: Vec<String>,

        /// Preserve level for existing dependencies
        #[arg(long, value_enum)]
        preserve: Option<PreserveLevel>,

        /// Add packages as weak dependencies
        #[arg(short = 'w', long)]
        weak: bool,

        /// Add packages as extra dependencies
        #[arg(short = 'e', long)]
        extra: bool,
    },

    /// Run the build script for packages
    Build {
        /// Packages to build (all if empty)
        packages: Vec<String>,

        /// Redirect build output to stdout/stderr
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// Edit compat entries in the current Project
    Compat {
        /// Package name
        package: Option<String>,

        /// Compat string
        compat_string: Option<String>,
    },

    /// Clone the full package repo locally for development
    #[command(visible_alias = "dev")]
    Develop {
        /// Package specifications or paths to develop
        packages: Vec<String>,

        /// Clone package to local project dev folder
        #[arg(short = 'l', long)]
        local: bool,

        /// Clone package to shared dev folder (default)
        #[arg(long)]
        shared: bool,

        /// Preserve level for existing dependencies
        #[arg(long, value_enum)]
        preserve: Option<PreserveLevel>,
    },

    /// Free pinned or developed packages
    Free {
        /// Packages to free (all if empty)
        packages: Vec<String>,

        /// Free all packages
        #[arg(long)]
        all: bool,
    },

    /// Generate files for packages
    Generate {
        /// Package name
        package: String,
    },

    /// Garbage collect packages not used for a significant time
    Gc {
        /// Show verbose output
        #[arg(short = 'v', long)]
        verbose: bool,

        /// Delete all packages that cannot be reached from any existing environment
        #[arg(long)]
        all: bool,
    },

    /// Download and install all artifacts in the manifest
    Instantiate {
        /// Instantiate project in verbose mode
        #[arg(short = 'v', long)]
        verbose: bool,

        /// Use manifest mode
        #[arg(short = 'm', long)]
        manifest: bool,

        /// Use project mode
        #[arg(short = 'p', long)]
        project: bool,
    },

    /// Pin packages
    Pin {
        /// Packages to pin (all if empty)
        packages: Vec<String>,

        /// Pin all packages
        #[arg(short = 'a', long)]
        all: bool,
    },

    /// Precompile packages
    Precompile {
        /// Packages to precompile (all if empty)
        packages: Vec<String>,
    },

    /// Remove packages from project
    #[command(visible_alias = "rm")]
    Remove {
        /// Packages to remove
        packages: Vec<String>,

        /// Use project mode
        #[arg(short = 'p', long)]
        project: bool,

        /// Use manifest mode
        #[arg(short = 'm', long)]
        manifest: bool,

        /// Remove all packages
        #[arg(long)]
        all: bool,
    },

    /// Registry operations
    Registry {
        #[command(subcommand)]
        command: Option<RegistryCommand>,
    },

    /// Resolve versions in the manifest
    Resolve,

    /// Show project status
    #[command(visible_alias = "st")]
    Status {
        /// Packages to show status for (all if empty)
        packages: Vec<String>,

        /// Show project compatibility status
        #[arg(short = 'c', long)]
        compat: bool,

        /// Show extension dependencies
        #[arg(short = 'e', long)]
        extensions: bool,

        /// Show manifest status instead of project status
        #[arg(short = 'm', long)]
        manifest: bool,

        /// Show diff between manifest and project
        #[arg(short = 'd', long)]
        diff: bool,

        /// Show status of outdated packages
        #[arg(short = 'o', long)]
        outdated: bool,
    },

    /// Run tests for packages
    Test {
        /// Packages to test (all if empty)
        packages: Vec<String>,

        /// Run tests with coverage enabled
        #[arg(long)]
        coverage: bool,
    },

    /// Update packages in manifest
    #[command(visible_alias = "up")]
    Update {
        /// Packages to update (all if empty)
        packages: Vec<String>,

        /// Use project mode
        #[arg(short = 'p', long)]
        project: bool,

        /// Use manifest mode
        #[arg(short = 'm', long)]
        manifest: bool,

        /// Only update within major version
        #[arg(long)]
        major: bool,

        /// Only update within minor version
        #[arg(long)]
        minor: bool,

        /// Only update within patch version
        #[arg(long)]
        patch: bool,

        /// Do not update
        #[arg(long)]
        fixed: bool,

        /// Preserve level for existing dependencies
        #[arg(long, value_enum)]
        preserve: Option<UpdatePreserveLevel>,
    },

    /// Explains why a package is in the dependency graph
    #[command(name = "why")]
    Why {
        /// Package name to explain
        package: String,
    },

    /// Generate shell completion scripts
    #[command(name = "completions")]
    Completions {
        #[arg(value_enum, value_name = "SHELL")]
        shell: CompletionShell,
    },
}

#[derive(Subcommand)]
enum RegistryCommand {
    /// Add package registries
    Add {
        /// Registry name or URL
        registry: String,
    },

    /// Remove package registries
    #[command(visible_alias = "rm")]
    Remove {
        /// Registry name or UUID
        registry: String,
    },

    /// Update package registries
    #[command(visible_alias = "up")]
    Update {
        /// Registries to update (all if empty)
        registries: Vec<String>,
    },

    /// Information about installed registries
    #[command(visible_alias = "st")]
    Status,
}

/// Parsed arguments structure
struct ParsedArgs {
    julia_flags: Vec<String>,
    channel: Option<String>,
    pkg_args: Vec<String>,
}

/// Parse command line arguments into Julia flags and Pkg commands
fn parse_arguments(args: &[String]) -> ParsedArgs {
    let mut julia_flags = Vec::new();
    let mut pkg_cmd_start = None;
    let mut channel = None;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];

        // Check for channel specifier
        if arg.starts_with('+') && arg.len() > 1 && channel.is_none() && pkg_cmd_start.is_none() {
            channel = Some(arg[1..].to_string());
            i += 1;
        }
        // Check for help flag
        else if arg == "--help" || arg == "-h" {
            if pkg_cmd_start.is_none() {
                // This is a help flag for jlpkg itself
                pkg_cmd_start = Some(i);
                break;
            }
            // Otherwise let it be part of pkg command
            pkg_cmd_start = Some(i);
            break;
        }
        // Check if this is a flag
        else if arg.starts_with('-') && pkg_cmd_start.is_none() {
            julia_flags.push(arg.clone());
            // If it's a flag with a value (e.g., --project=...), it's already included
            // If it's a flag that expects a value next (e.g., --project ...), get the next arg
            if !arg.contains('=') && i + 1 < args.len() {
                let next_arg = &args[i + 1];
                if !next_arg.starts_with('-') && !next_arg.starts_with('+') {
                    julia_flags.push(next_arg.clone());
                    i += 1; // Skip the next arg since we've consumed it
                }
            }
            i += 1;
        }
        // This is the start of pkg commands
        else {
            pkg_cmd_start = Some(i);
            break;
        }
    }

    let pkg_args = if let Some(start) = pkg_cmd_start {
        args[start..].to_vec()
    } else {
        vec![]
    };

    ParsedArgs {
        julia_flags,
        channel,
        pkg_args,
    }
}

/// Show help message and exit
fn show_help() -> Result<std::process::ExitCode> {
    match Cli::try_parse_from(["jlpkg", "--help"]) {
        Ok(_) => {}
        Err(e) => {
            // Clap returns an error for --help but prints to stderr
            // We print to stdout for consistency with other CLIs
            println!("{}", e);
        }
    }
    Ok(std::process::ExitCode::from(0))
}

/// Validate Pkg command with clap
fn validate_pkg_command(pkg_args: &[String]) -> Result<()> {
    let mut parse_args = vec!["jlpkg".to_string()];
    parse_args.extend(pkg_args.iter().cloned());

    match Cli::try_parse_from(&parse_args) {
        Ok(_) => Ok(()),
        Err(e) => {
            // Check if this is a help request
            if e.kind() == clap::error::ErrorKind::DisplayHelp
                || e.kind() == clap::error::ErrorKind::DisplayVersion
            {
                println!("{}", e);
                std::process::exit(0);
            }
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

/// Build the final Julia command arguments
fn build_julia_args(args: &[String], parsed: &ParsedArgs) -> Vec<String> {
    let mut new_args = Vec::new();

    // Add the executable name
    new_args.push(args[0].clone());

    // Add channel if specified
    if let Some(ch) = &parsed.channel {
        new_args.push(format!("+{}", ch));
    }

    // Define default flags for jlpkg
    let defaults = [
        ("--startup-file", "no"),
        ("--project", "."),
        ("--threads", "auto"),
    ];

    // Add Julia flags
    new_args.extend(parsed.julia_flags.clone());

    // Add default flags if not already specified
    for (flag, value) in defaults {
        // Check if this flag is already specified
        let already_specified = if flag == "--threads" {
            // Check for both --threads and -t
            parsed
                .julia_flags
                .iter()
                .any(|f| f.starts_with("--threads") || f.starts_with("-t"))
        } else {
            parsed.julia_flags.iter().any(|f| f.starts_with(flag))
        };

        if !already_specified {
            new_args.push(format!("{}={}", flag, value));
        }
    }

    // Add the Pkg command execution
    let pkg_cmd_str = parsed.pkg_args.join(" ");
    new_args.push("-e".to_string());
    new_args.push(format!("using Pkg; isdefined(Pkg.REPLMode, :PRINTED_REPL_WARNING) && (Pkg.REPLMode.PRINTED_REPL_WARNING[] = true); Pkg.REPLMode.pkgstr(\"{}\")", pkg_cmd_str));

    new_args
}

fn main() -> Result<std::process::ExitCode> {
    let args: Vec<String> = std::env::args().collect();

    // Handle the case where only jlpkg is called
    if args.len() == 1 {
        return show_help();
    }

    // Parse arguments
    let parsed = parse_arguments(&args);

    // Handle help flag in arguments
    if parsed.pkg_args.first().map(|s| s.as_str()) == Some("--help")
        || parsed.pkg_args.first().map(|s| s.as_str()) == Some("-h")
    {
        return show_help();
    }

    // If there are no pkg commands, show help
    if parsed.pkg_args.is_empty() {
        return show_help();
    }

    // Check if this is the completions command
    if parsed.pkg_args.first().map(|s| s.as_str()) == Some("completions") {
        // Parse the completions command
        let mut parse_args = vec!["jlpkg".to_string()];
        parse_args.extend(parsed.pkg_args.iter().cloned());

        match Cli::try_parse_from(&parse_args) {
            Ok(cli) => {
                if let Some(PkgCommand::Completions { shell }) = cli.command {
                    if let Err(e) =
                        juliaup::command_completions::generate_jlpkg_completions::<Cli>(shell)
                    {
                        eprintln!("Error generating completions: {}", e);
                        return Ok(std::process::ExitCode::from(1));
                    }
                    return Ok(std::process::ExitCode::from(0));
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                return Ok(std::process::ExitCode::from(1));
            }
        }
    }

    // Validate the Pkg command
    validate_pkg_command(&parsed.pkg_args)?;

    // Build the final Julia command arguments
    let new_args = build_julia_args(&args, &parsed);

    // Replace the current process args and call the shared launcher
    std::env::set_var("JULIA_PROGRAM_OVERRIDE", "jlpkg");
    let exit_code = juliaup::julia_launcher::run_julia_launcher(new_args, None)?;
    Ok(std::process::ExitCode::from(exit_code as u8))
}
