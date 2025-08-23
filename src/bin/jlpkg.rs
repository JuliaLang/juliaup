use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use juliaup::julia_path_utils::JuliaupEnvironment;
use std::process::Command;

#[derive(Parser)]
#[command(name = "jlpkg")]
#[command(about = "Julia package manager interface", long_about = None)]
struct Cli {
    /// Select Julia channel (e.g., +1.11, +release)
    #[arg(value_name = "CHANNEL", value_parser = parse_channel)]
    channel: Option<String>,

    #[command(subcommand)]
    command: PkgCommand,
}

fn parse_channel(s: &str) -> Result<String, String> {
    if let Some(stripped) = s.strip_prefix('+') {
        Ok(stripped.to_string())
    } else {
        Err(format!("Channel must start with '+', got: {}", s))
    }
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

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    // Extract channel and Julia flags before clap parsing
    let mut channel: Option<String> = None;
    let mut julia_flags: Vec<String> = Vec::new();
    let mut clap_args = vec![args[0].clone()];
    let mut found_subcommand = false;
    
    for arg in args.iter().skip(1) {
        if let Some(stripped) = arg.strip_prefix('+') {
            channel = Some(stripped.to_string());
        } else if !found_subcommand && (arg.starts_with("--project") || 
                                         arg.starts_with("--startup-file") || 
                                         arg.starts_with("--color") ||
                                         arg.starts_with("-J") ||
                                         arg.starts_with("--sysimage") ||
                                         arg.starts_with("--threads") ||
                                         arg.starts_with("-t")) {
            // This is a Julia flag, save it for later
            julia_flags.push(arg.clone());
        } else {
            // Check if this is a known subcommand
            if !found_subcommand && matches!(arg.as_str(), 
                "add" | "build" | "compat" | "develop" | "dev" | 
                "free" | "gc" | "generate" | "instantiate" | "pin" | 
                "precompile" | "remove" | "rm" | "resolve" | "status" | 
                "st" | "test" | "update" | "up" | "why" | "registry") {
                found_subcommand = true;
            }
            clap_args.push(arg.clone());
        }
    }
    
    // Parse remaining args with clap
    let cli = Cli::parse_from(&clap_args);
    
    // Use channel from CLI if provided, otherwise from extraction above
    let channel = cli.channel.or(channel);
    
    // Build the Pkg command string
    let pkg_command = match cli.command {
        PkgCommand::Add { packages } => {
            format!("add {}", packages.join(" "))
        }
        PkgCommand::Build { packages } => {
            if packages.is_empty() {
                "build".to_string()
            } else {
                format!("build {}", packages.join(" "))
            }
        }
        PkgCommand::Compat { package, version } => {
            let mut cmd = "compat".to_string();
            if let Some(p) = package {
                cmd.push_str(&format!(" {}", p));
                if let Some(v) = version {
                    cmd.push_str(&format!(" {}", v));
                }
            }
            cmd
        }
        PkgCommand::Develop { packages, local, shared } => {
            let mut cmd = "develop".to_string();
            if local {
                cmd.push_str(" --local");
            }
            if shared {
                cmd.push_str(" --shared");
            }
            if !packages.is_empty() {
                cmd.push_str(&format!(" {}", packages.join(" ")));
            }
            cmd
        }
        PkgCommand::Free { packages } => {
            format!("free {}", packages.join(" "))
        }
        PkgCommand::Gc { all } => {
            if all {
                "gc --all".to_string()
            } else {
                "gc".to_string()
            }
        }
        PkgCommand::Generate { package_name } => {
            format!("generate {}", package_name)
        }
        PkgCommand::Instantiate => "instantiate".to_string(),
        PkgCommand::Pin { packages } => {
            format!("pin {}", packages.join(" "))
        }
        PkgCommand::Precompile => "precompile".to_string(),
        PkgCommand::Remove { packages } => {
            format!("remove {}", packages.join(" "))
        }
        PkgCommand::Resolve => "resolve".to_string(),
        PkgCommand::Status { diff, outdated, manifest } => {
            let mut cmd = "status".to_string();
            if diff {
                cmd.push_str(" --diff");
            }
            if outdated {
                cmd.push_str(" --outdated");
            }
            if manifest {
                cmd.push_str(" --manifest");
            }
            cmd
        }
        PkgCommand::Test { packages, coverage } => {
            let mut cmd = "test".to_string();
            if !packages.is_empty() {
                cmd.push_str(&format!(" {}", packages.join(" ")));
            }
            if coverage {
                cmd.push_str(" --coverage");
            }
            cmd
        }
        PkgCommand::Update { packages } => {
            if packages.is_empty() {
                "update".to_string()
            } else {
                format!("update {}", packages.join(" "))
            }
        }
        PkgCommand::Why { package } => {
            format!("why {}", package)
        }
        PkgCommand::Registry { command } => {
            match command {
                RegistryCommand::Add { registry } => {
                    format!("registry add {}", registry)
                }
                RegistryCommand::Remove { registry } => {
                    format!("registry remove {}", registry)
                }
                RegistryCommand::Status => "registry status".to_string(),
                RegistryCommand::Update { registry } => {
                    if let Some(r) = registry {
                        format!("registry update {}", r)
                    } else {
                        "registry update".to_string()
                    }
                }
            }
        }
    };
    
    // Load juliaup environment and get Julia binary path
    let env = JuliaupEnvironment::load()?;
    let (julia_path, _julia_args) = env.get_julia_path(channel, false)?;
    
    // Build and execute the Julia command
    let julia_code = format!(
        "using Pkg; isdefined(Pkg.REPLMode, :PRINTED_REPL_WARNING) && (Pkg.REPLMode.PRINTED_REPL_WARNING[] = true); Pkg.REPLMode.pkgstr(\"{}\")", 
        pkg_command
    );
    
    let mut cmd = Command::new(&julia_path);
    
    let default_flags = [
        ("--project", "."),
        ("--startup-file", "no"),
        ("--color", "yes"),
    ];
    
    // Add defaults only if user hasn't provided them
    for (flag, value) in default_flags {
        if !julia_flags.iter().any(|f| f.starts_with(flag)) {
            cmd.arg(format!("{}={}", flag, value));
        }
    }
    
    // Add user-provided Julia flags
    for flag in &julia_flags {
        cmd.arg(flag);
    }
    
    // Add the actual command
    cmd.arg("-e");
    cmd.arg(&julia_code);
    
    let status = cmd
        .status()
        .with_context(|| format!("Failed to execute Julia at {}", julia_path.display()))?;
    
    std::process::exit(status.code().unwrap_or(1));
}