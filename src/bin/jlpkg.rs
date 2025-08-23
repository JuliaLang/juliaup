use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "jlpkg")]
#[command(about = "Julia package manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: PkgCommand,
}

#[derive(Subcommand)]
enum PkgCommand {
    /// Add packages to project
    Add {
        /// Package specifications to add
        packages: Vec<String>,
    },
    /// Run the build script for packages
    Build {
        /// Packages to build
        packages: Vec<String>,
    },
    /// Edit compat entries in the current Project and re-resolve
    Compat {
        /// Package name
        package: Option<String>,
        /// Version specification
        version: Option<String>,
    },
    /// Clone the full package repo locally for development
    #[command(visible_alias = "dev")]
    Develop {
        /// Package specifications to develop
        packages: Vec<String>,
        /// Use local path
        #[arg(long)]
        local: bool,
        /// Use shared environment
        #[arg(long)]
        shared: bool,
    },
    /// Undoes a pin, develop, or stops tracking a repo
    Free {
        /// Packages to free
        packages: Vec<String>,
    },
    /// Garbage collect packages not used for a significant time
    Gc {
        /// Collect all packages
        #[arg(long)]
        all: bool,
    },
    /// Generate files for a new project
    Generate {
        /// Package name
        package_name: String,
    },
    /// Downloads all the dependencies for the project
    Instantiate,
    /// Pins the version of packages
    Pin {
        /// Packages to pin
        packages: Vec<String>,
    },
    /// Precompile all the project dependencies
    Precompile,
    /// Remove packages from project or manifest
    #[command(visible_alias = "rm")]
    Remove {
        /// Packages to remove
        packages: Vec<String>,
    },
    /// Resolves to update the manifest from changes in dependencies
    Resolve,
    /// Summarize contents of and changes to environment
    #[command(visible_alias = "st")]
    Status {
        /// Show diff
        #[arg(long)]
        diff: bool,
        /// Show outdated packages
        #[arg(long)]
        outdated: bool,
        /// Show manifest
        #[arg(long)]
        manifest: bool,
    },
    /// Run tests for packages
    Test {
        /// Packages to test
        packages: Vec<String>,
        /// Test coverage
        #[arg(long)]
        coverage: bool,
    },
    /// Update packages in manifest
    #[command(visible_alias = "up")]
    Update {
        /// Packages to update
        packages: Vec<String>,
    },
    /// Shows why a package is in the manifest
    Why {
        /// Package name
        package: String,
    },
    /// Registry operations
    Registry {
        #[command(subcommand)]
        command: RegistryCommand,
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
        /// Registry name
        registry: String,
    },
    /// Information about installed registries
    #[command(visible_alias = "st")]
    Status,
    /// Update package registries
    #[command(visible_alias = "up")]
    Update {
        /// Registry name (optional)
        registry: Option<String>,
    },
}

fn main() -> Result<std::process::ExitCode> {
    // Get original args
    let args: Vec<String> = std::env::args().collect();
    
    // Find where the Pkg command starts (first non-flag, non-channel argument)
    let mut pkg_cmd_start = None;
    let mut julia_args = vec![args[0].clone()]; // Program name
    let mut help_requested = false;
    
    for (idx, arg) in args.iter().enumerate().skip(1) {
        if arg == "--help" || arg == "-h" {
            help_requested = true;
            break;
        } else if arg.starts_with('+') || arg.starts_with('-') {
            // This is a channel selector or Julia flag, pass it through
            julia_args.push(arg.clone());
        } else {
            // This is the start of the Pkg command
            pkg_cmd_start = Some(idx);
            break;
        }
    }
    
    // If help requested or no command, show clap help
    if help_requested || pkg_cmd_start.is_none() {
        // Use clap just for the help message
        // This will print help and exit
        Cli::parse();
        std::process::exit(0);
    }
    
    // Build the Pkg command string from remaining args
    let pkg_command = args[pkg_cmd_start.unwrap()..].join(" ");
    
    // Add default --project=. if not specified
    if !julia_args.iter().any(|arg| arg.starts_with("--project")) {
        julia_args.push("--project=.".to_string());
    }
    
    // Add default Julia flags
    julia_args.push("--startup-file=no".to_string());
    julia_args.push("--color=yes".to_string());
    
    // Add the -e flag with the Pkg.REPLMode.pkgstr command
    julia_args.push("-e".to_string());
    julia_args.push(format!(
        "using Pkg; isdefined(Pkg.REPLMode, :PRINTED_REPL_WARNING) && (Pkg.REPLMode.PRINTED_REPL_WARNING[] = true); Pkg.REPLMode.pkgstr(\"{}\")",
        pkg_command.replace('\\', "\\\\").replace('"', "\\\"")
    ));
    
    // Use the shared launcher
    let client_status = juliaup::julia_launcher::run_julia_launcher(julia_args, Some("Julia (Pkg)"));
    
    if let Err(_err) = &client_status {
        // The launcher will handle error printing
        return Err(client_status.unwrap_err());
    }
    
    // TODO https://github.com/rust-lang/rust/issues/111688 is finalized, we should use that instead of calling exit
    std::process::exit(client_status?);
}