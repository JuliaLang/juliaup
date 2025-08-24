use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "jlpkg")]
#[command(about = "Julia package manager", long_about = None)]
#[command(allow_external_subcommands = true)]
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

        /// Only download the package
        #[arg(short = 'l', long)]
        local: bool,

        /// Install the dependencies of the package
        #[arg(short = 'd', long)]
        shared: bool,

        /// Preserve level for existing dependencies
        #[arg(long, value_enum)]
        preserve: Option<PreserveLevel>,
    },

    /// Free packages from being developed
    Free {
        /// Packages to free (all if empty)
        packages: Vec<String>,
    },

    /// Generate files for packages
    Generate {
        /// Package name
        package: String,

        /// Generate package in its own directory
        #[arg(short = 't', long)]
        template: bool,
    },

    /// Garbage collect packages not used for a significant time
    Gc {
        /// Delete all packages that cannot be reached from any existing environment
        #[arg(long)]
        all: bool,

        /// Only log packages that would be garbage collected
        #[arg(long)]
        dry_run: bool,
    },

    /// Download and install all artifacts in the manifest
    Instantiate {
        /// Instantiate project in verbose mode
        #[arg(short = 'v', long)]
        verbose: bool,

        /// Manifest file to instantiate
        #[arg(short = 'm', long, value_name = "PATH")]
        manifest: Option<String>,

        /// Project directory
        #[arg(short = 'p', long, value_name = "PATH", id = "proj")]
        project: Option<String>,
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

        /// Force recompilation
        #[arg(long)]
        force: bool,

        /// Precompile for different configuration
        #[arg(long)]
        check_bounds: Option<String>,

        /// Precompile for inlining or not
        #[arg(long)]
        inline: Option<bool>,

        /// Precompile package dependencies in parallel
        #[arg(short = 'j', long)]
        jobs: Option<usize>,

        /// Precompile all configurations
        #[arg(long)]
        all: bool,

        /// Precompile in strict mode
        #[arg(long)]
        strict: bool,

        /// Warn when precompiling
        #[arg(long)]
        warn_loaded: bool,

        /// Only check if packages need precompilation
        #[arg(long)]
        already_instantiated: bool,
    },

    /// Remove packages from project
    #[command(visible_alias = "rm")]
    Remove {
        /// Packages to remove
        packages: Vec<String>,

        /// Update manifest
        #[arg(short = 'u', long)]
        update: bool,

        /// Remove mode
        #[arg(short = 'm', long, value_name = "manifest|project|deps|all")]
        mode: Option<String>,

        /// Remove all packages
        #[arg(short = 'a', long)]
        all: bool,
    },

    /// Registry operations
    Registry {
        #[command(subcommand)]
        command: Option<RegistryCommand>,
    },

    /// Resolve versions in the manifest
    Resolve {
        /// Packages to resolve
        packages: Vec<String>,
    },

    /// Show project status
    #[command(visible_alias = "st")]
    Status {
        /// Packages to show status for (all if empty)
        packages: Vec<String>,

        /// Show project compatibility status
        #[arg(short = 'c', long)]
        compat: bool,

        /// Show test dependency compatibility status
        #[arg(short = 't', long)]
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

        /// Show status as a table
        #[arg(long)]
        as_table: bool,
    },

    /// Run tests for packages
    Test {
        /// Packages to test (all if empty)
        packages: Vec<String>,

        /// Set code coverage to track
        #[arg(long, value_name = "none|user|all")]
        coverage: Option<String>,
    },

    /// Update packages in manifest
    #[command(visible_alias = "up")]
    Update {
        /// Packages to update (all if empty)
        packages: Vec<String>,

        /// Preserve level for existing dependencies
        #[arg(long, value_enum)]
        preserve: Option<PreserveLevel>,

        /// Update manifest
        #[arg(short = 'm', long)]
        manifest: bool,
    },

    /// Preview a registry package
    Preview {
        /// Package name
        package: String,
    },

    /// Explains why a package is in the dependency graph
    #[command(name = "why")]
    Why {
        /// Package name to explain
        package: String,
    },

    /// Clean the Julia cache
    Clean,
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

fn main() -> Result<std::process::ExitCode> {
    // Collect all args
    let args: Vec<String> = std::env::args().collect();

    // Handle the case where only jlpkg is called
    if args.len() == 1 {
        // Show help by passing --help to clap
        match Cli::try_parse_from(&["jlpkg", "--help"]) {
            Ok(_) => {}
            Err(e) => {
                // Clap returns an error for --help but prints to stderr
                // We print to stdout for consistency with other CLIs
                println!("{}", e);
                return Ok(std::process::ExitCode::from(0));
            }
        }
        return Ok(std::process::ExitCode::from(0));
    }

    // Separate Julia flags from Pkg commands
    let mut julia_flags = Vec::new();
    let mut pkg_cmd_start = None;
    let mut channel = None;

    for (i, arg) in args[1..].iter().enumerate() {
        // Check for channel specifier
        if arg.starts_with('+') && arg.len() > 1 && channel.is_none() && pkg_cmd_start.is_none() {
            channel = Some(arg[1..].to_string());
        }
        // Check for help flag
        else if arg == "--help" || arg == "-h" {
            if pkg_cmd_start.is_none() {
                // Show jlpkg help
                match Cli::try_parse_from(&["jlpkg", "--help"]) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("{}", e);
                        return Ok(std::process::ExitCode::from(0));
                    }
                }
                return Ok(std::process::ExitCode::from(0));
            }
            // Otherwise let it be part of pkg command
            break;
        }
        // Check if this is a flag
        else if arg.starts_with('-') && pkg_cmd_start.is_none() {
            julia_flags.push(arg.clone());
            // If it's a flag with a value (e.g., --project=...), it's already included
            // If it's a flag that expects a value next (e.g., --project ...), get the next arg
            if !arg.contains('=') && i + 1 < args.len() - 1 {
                let next_arg = &args[i + 2];
                if !next_arg.starts_with('-') {
                    julia_flags.push(next_arg.clone());
                }
            }
        }
        // This is the start of pkg commands
        else if pkg_cmd_start.is_none() {
            pkg_cmd_start = Some(i + 1);
            break;
        }
    }

    // If there are no pkg commands, show help
    let pkg_args = if let Some(start) = pkg_cmd_start {
        args[start..].to_vec()
    } else {
        vec![]
    };

    if pkg_args.is_empty() {
        // Show help
        let _ = Cli::try_parse_from(&["jlpkg", "--help"]);
        return Ok(std::process::ExitCode::from(0));
    }

    // Parse pkg command with clap for validation
    let mut parse_args = vec!["jlpkg".to_string()];
    parse_args.extend(pkg_args.clone());

    match Cli::try_parse_from(&parse_args) {
        Ok(_) => {
            // Command is valid, continue
        }
        Err(e) => {
            // Check if this is a help request
            if e.kind() == clap::error::ErrorKind::DisplayHelp
                || e.kind() == clap::error::ErrorKind::DisplayVersion
            {
                println!("{}", e);
                return Ok(std::process::ExitCode::from(0));
            }
            eprintln!("{}", e);
            return Ok(std::process::ExitCode::from(1));
        }
    };

    // Use the original pkg arguments as-is
    let pkg_cmd_str = pkg_args.join(" ");

    // Build Julia arguments
    let mut new_args = Vec::new();

    // Add the executable name
    new_args.push(args[0].clone());

    // Add channel if specified
    if let Some(ch) = channel {
        new_args.push(format!("+{}", ch));
    }

    // Define default flags for jlpkg
    let defaults = [
        ("--color", "yes"),
        ("--startup-file", "no"),
        ("--project", "."),
    ];

    // Add Julia flags
    new_args.extend(julia_flags.clone());

    // Add default flags if not already specified
    for (flag, value) in defaults {
        // Check if this flag (or its underscore variant) is already specified
        let flag_underscore = flag.replace('-', "_");
        if !julia_flags
            .iter()
            .any(|f| f.starts_with(flag) || f.starts_with(&flag_underscore))
        {
            new_args.push(format!("{}={}", flag, value));
        }
    }

    // Add the Pkg command execution
    new_args.push("-e".to_string());
    new_args.push(format!("using Pkg; isdefined(Pkg.REPLMode, :PRINTED_REPL_WARNING) && (Pkg.REPLMode.PRINTED_REPL_WARNING[] = true); Pkg.REPLMode.pkgstr(\"{}\")", pkg_cmd_str));

    // Replace the current process args and call the shared launcher
    std::env::set_var("JULIA_PROGRAM_OVERRIDE", "jlpkg");
    let exit_code = juliaup::julia_launcher::run_julia_launcher(new_args, None)?;
    Ok(std::process::ExitCode::from(exit_code as u8))
}
