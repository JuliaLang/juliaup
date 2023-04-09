use anyhow::Result;
use clap::Parser;

#[cfg(feature = "selfupdate")]
fn run_individual_config_wizard(
    install_choices: &mut InstallChoices,
    theme: &dyn dialoguer::theme::Theme,
) -> Result<Option<()>> {
    use std::path::PathBuf;

    use dialoguer::{Confirm, Input};
    use log::trace;

    trace!(
        "install_location pre inside the prompt function {:?}",
        install_choices.install_location
    );

    let new_install_location = Input::with_theme(theme)
        .with_prompt("Enter the folder where you want to install Juliaup")
        .validate_with(|input: &String| match input.parse::<PathBuf>() {
            Ok(_) => Ok(()),
            Err(_) => Err("Not a valid input".to_owned()),
        })
        .with_initial_text(install_choices.install_location.to_string_lossy().clone())
        .interact_text()?;

    let new_install_location = shellexpand::tilde(&new_install_location)
        .parse::<PathBuf>()
        .unwrap();

    let new_modifypath = match Confirm::with_theme(theme)
        .with_prompt("Do you want to add the Julia binaries to your PATH by manipulating various shell startup scripts?")
        .default(install_choices.modifypath)
        .interact_opt()? {
            Some(value) => value,
            None => return Ok(None)
        };

    let new_symlinks = match Confirm::with_theme(theme)
        .with_prompt("Do you want to add channel specific symlinks?")
        .default(install_choices.symlinks)
        .interact_opt()?
    {
        Some(value) => value,
        None => return Ok(None),
    };

    let new_startupselfupdate = Input::with_theme(theme)
        .with_prompt(
            "Enter minutes between check for new version at julia startup, use 0 to disable",
        )
        .validate_with(|input: &String| -> Result<(), &str> {
            match input.parse::<i64>() {
                Ok(val) => {
                    if val >= 0 {
                        Ok(())
                    } else {
                        Err("Not a valid input")
                    }
                }
                Err(_) => Err("Not a valid input"),
            }
        })
        .default(install_choices.startupselfupdate.to_string())
        .interact_text()?
        .parse::<i64>()
        .unwrap();

    let new_backgroundselfupdate = Input::with_theme(theme)
        .with_prompt(
            "Enter minutes between check for new version by a background task, use 0 to disable",
        )
        .validate_with(|input: &String| -> Result<(), &str> {
            match input.parse::<i64>() {
                Ok(val) => {
                    if val >= 0 {
                        Ok(())
                    } else {
                        Err("Not a valid input")
                    }
                }
                Err(_) => Err("Not a valid input"),
            }
        })
        .default(install_choices.backgroundselfupdate.to_string())
        .interact_text()?
        .parse::<i64>()
        .unwrap();

    install_choices.install_location = new_install_location;
    install_choices.modifypath = new_modifypath;
    install_choices.symlinks = new_symlinks;
    install_choices.startupselfupdate = new_startupselfupdate;
    install_choices.backgroundselfupdate = new_backgroundselfupdate;

    Ok(Some(()))
}

#[cfg(feature = "selfupdate")]
fn is_juliaup_installed() -> bool {
    use std::process::Stdio;

    let exit_status = std::process::Command::new("juliaup")
        .args(["--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .status();

    exit_status.is_ok()
}

#[derive(Parser)]
#[clap(name = "Juliainstaller", version)]
/// The Julia Installer
struct Juliainstaller {
    /// Default channel
    #[clap(long, default_value = "release")]
    default_channel: String,
    /// Juliaup channel
    #[clap(long, default_value = "release")]
    juliaup_channel: String,
    /// Disable confirmation prompt
    #[clap(short = 'y', long = "yes")]
    disable_confirmation_prompt: bool,
}

#[cfg(feature = "selfupdate")]
struct InstallChoices {
    backgroundselfupdate: i64,
    startupselfupdate: i64,
    symlinks: bool,
    modifypath: bool,
    install_location: std::path::PathBuf,
    modifypath_files: Vec<std::path::PathBuf>,
}

#[cfg(feature = "selfupdate")]
fn print_install_choices(install_choices: &InstallChoices) -> Result<()> {
    use console::style;

    println!("Juliaup will be installed into the Juliaup home directory, located at:");
    println!();
    println!("  {}", install_choices.install_location.to_string_lossy());
    println!();
    println!(
        "The {}, {} and other commands will be added to",
        style("julia").bold(),
        style("juliaup").bold()
    );
    println!("Juliaup's bin directory, located at:");
    println!();
    println!(
        "  {}",
        install_choices
            .install_location
            .join("bin")
            .to_string_lossy()
    );
    println!();

    if install_choices.modifypath {
        println!(
            "This path will then be added to your {} environment variable by",
            style("PATH").bold()
        );
        println!("modifying the profile files located at:");
        println!();
        for p in &install_choices.modifypath_files {
            println!("  {}", p.to_string_lossy());
        }
        println!();
    }

    if install_choices.backgroundselfupdate > 0 {
        println!("The installer will configure a CRON job that checks for updates of");
        println!(
            "Juliaup itself. This CRON job will run every {} seconds.",
            install_choices.backgroundselfupdate
        );
        println!();
    }

    if install_choices.startupselfupdate > 0 {
        println!("Julia will look for a new version of Juliaup itself every {} minutes when you start julia.", install_choices.startupselfupdate);
        println!();
    }

    if install_choices.symlinks {
        println!("Julia will create a symlink for every channel you install that is named julia-<CHANNELNAME>.");
        println!();
    }

    Ok(())
}

#[cfg(feature = "selfupdate")]
pub fn main() -> Result<()> {
    use anyhow::{anyhow, Context};
    use console::{style, Style};
    use dialoguer::{
        theme::{ColorfulTheme, SimpleTheme, Theme},
        Confirm, Select,
    };
    use juliaup::{
        command_add::run_command_add,
        command_default::run_command_default,
        command_selfchannel::run_command_selfchannel,
        config_file::JuliaupSelfConfig,
        get_juliaup_target, get_own_version,
        global_paths::get_paths,
        operations::{download_extract_sans_parent, find_shell_scripts_to_be_modified},
        utils::get_juliaserver_base_url,
    };
    use std::io::Seek;

    human_panic::setup_panic!(human_panic::Metadata {
        name: "Juliainstaller".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        authors: "".into(),
        homepage: "https://github.com/JuliaLang/juliaup".into(),
    });

    let env = env_logger::Env::new()
        .filter("JULIAUP_LOG")
        .write_style("JULIAUP_LOG_STYLE");
    env_logger::init_from_env(env);

    info!("Parsing command line arguments.");
    let args = Juliainstaller::parse();

    if !args.disable_confirmation_prompt && !is_terminal::is_terminal(&std::io::stdin()) {
        return Err(anyhow!(
            "To install Julia in non-interactive mode pass the -y parameter."
        ));
    }

    let theme: Box<dyn Theme> = if is_terminal::is_terminal(&std::io::stdout()) {
        Box::new(ColorfulTheme {
            values_style: Style::new().yellow().dim(),
            ..ColorfulTheme::default()
        })
    } else {
        Box::new(SimpleTheme)
    };

    let mut paths = get_paths().with_context(|| "Trying to load all global paths.")?;

    use juliaup::{
        command_config_backgroundselfupdate::run_command_config_backgroundselfupdate,
        command_config_modifypath::run_command_config_modifypath,
        command_config_startupselfupdate::run_command_config_startupselfupdate,
        command_config_symlinks::run_command_config_symlinks,
    };
    use log::{debug, info, trace};

    println!("{}", style("Welcome to Julia!").bold());
    println!();

    if is_juliaup_installed() {
        println!("It seems that Juliaup is already installed on this system. Please remove the previous installation of Juliaup before you try to install a new version.");

        return Ok(());
    }

    println!("This will download and install the official Julia Language distribution");
    println!("and its version manager Juliaup.");
    println!();

    if paths.juliaupconfig.exists() {
        println!("While Juliaup does not seem to be installed on this system, there is a");
        println!("Juliaup configuration file present from a previous installation.");

        if args.disable_confirmation_prompt {
            println!();
            println!(
                "Please remove the existing Juliaup configuration file or use interactive mode."
            );

            return Ok(());
        } else {
            let continue_with_setup = Confirm::with_theme(theme.as_ref())
                .with_prompt("Do you want to continue with the installation and overwrite the existing Juliaup configuration file?")
                .default(true)
                .interact_opt()?;

            if !continue_with_setup.unwrap_or(false) {
                return Ok(());
            }

            println!();
        }
    }

    let mut install_choices = InstallChoices {
        backgroundselfupdate: 0,
        startupselfupdate: 1440,
        symlinks: false,
        modifypath: true,
        install_location: dirs::home_dir()
            .ok_or(anyhow!(
                "Could not determine the path of the user home directory."
            ))?
            .join(".juliaup"),
        modifypath_files: find_shell_scripts_to_be_modified(true)
            .with_context(|| "Failed to identify the shell scripts that need to be modified.")?,
    };

    print_install_choices(&install_choices)?;

    println!(
        "You can uninstall at any time with {} and these",
        style("juliaup self uninstall").bold()
    );
    println!("changes will be reverted.");
    println!();

    if !args.disable_confirmation_prompt {
        debug!("Next running the prompt for default choices");

        let answer_default = Select::with_theme(theme.as_ref())
            .with_prompt("Do you want to install with these default configuration choices?")
            .item("Proceed with installation")
            .item("Customize installation")
            .item("Cancel installation")
            .default(0)
            .interact()?;

        trace!("choice is {:?}", answer_default);

        println!();

        if answer_default == 1 {
            debug!("Next running the individual config wizard");

            loop {
                run_individual_config_wizard(&mut install_choices, theme.as_ref())?;

                print_install_choices(&install_choices)?;

                let confirmcustom = Select::with_theme(theme.as_ref())
                    .with_prompt("Do you want to install with these custom configuration choices?")
                    .item("Proceed with installation")
                    .item("Customize installation")
                    .item("Cancel installation")
                    .default(0)
                    .interact()?;

                trace!("homedir is {:?}", install_choices.install_location);

                if confirmcustom == 0 {
                    break;
                } else if confirmcustom == 2 {
                    return Ok(());
                }
            }
        } else if answer_default == 2 {
            return Ok(());
        }
    }

    let juliaupselfbin = install_choices.install_location.join("bin");

    trace!("Set juliaupselfbin to `{:?}`", juliaupselfbin);

    println!("Now installing Juliaup");

    if paths.juliaupconfig.exists() {
        std::fs::remove_file(&paths.juliaupconfig).unwrap();
    }

    std::fs::create_dir_all(&juliaupselfbin)
        .with_context(|| "Failed to create install folder for Juliaup.")?;

    let juliaup_target = get_juliaup_target();

    let juliaupserver_base =
        get_juliaserver_base_url().with_context(|| "Failed to get Juliaup server base URL.")?;

    let version = get_own_version().unwrap();
    // let version = semver::Version::parse("1.5.29").unwrap();

    let download_url_path = format!("juliaup/bin/juliaup-{}-{}.tar.gz", version, juliaup_target);

    let new_juliaup_url = juliaupserver_base
        .join(&download_url_path)
        .with_context(|| {
            format!(
                "Failed to construct a valid url from '{}' and '{}'.",
                juliaupserver_base, download_url_path
            )
        })?;

    download_extract_sans_parent(&new_juliaup_url.to_string(), &juliaupselfbin, 0)?;

    {
        let new_selfconfig_data = JuliaupSelfConfig {
            background_selfupdate_interval: None,
            startup_selfupdate_interval: None,
            modify_path: false,
            juliaup_channel: None,
            last_selfupdate: None,
        };

        let self_config_path = install_choices.install_location.join("juliaupself.json");

        let mut self_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&self_config_path)
            .with_context(|| "Failed to open juliaup config file.")?;

        self_file
            .rewind()
            .with_context(|| "Failed to rewind self config file for write.")?;

        self_file.set_len(0).with_context(|| {
            "Failed to set len to 0 for self config file before writing new content."
        })?;

        serde_json::to_writer_pretty(&self_file, &new_selfconfig_data)
            .with_context(|| format!("Failed to write self configuration file."))?;

        self_file
            .sync_all()
            .with_context(|| "Failed to write config data to disc.")?;

        paths.juliaupselfbin = juliaupselfbin.clone();
        paths.juliaupselfconfig = self_config_path.clone();
    }

    run_command_config_backgroundselfupdate(
        Some(install_choices.backgroundselfupdate),
        true,
        &paths,
    )
    .unwrap();
    run_command_config_startupselfupdate(Some(install_choices.startupselfupdate), true, &paths)
        .unwrap();
    run_command_config_modifypath(Some(install_choices.modifypath), true, &paths).unwrap();
    run_command_config_symlinks(Some(install_choices.symlinks), true, &paths).unwrap();
    run_command_selfchannel(args.juliaup_channel, &paths).unwrap();

    run_command_add(&args.default_channel, &paths)
        .with_context(|| "Failed to run `run_command_add`.")?;

    run_command_default(&args.default_channel, &paths)
        .with_context(|| "Failed to run `run_command_default`.")?;

    let symlink_path = juliaupselfbin.join("julia");

    std::os::unix::fs::symlink(juliaupselfbin.join("julialauncher"), &symlink_path).with_context(
        || {
            format!(
                "failed to create symlink `{}`.",
                symlink_path.to_string_lossy()
            )
        },
    )?;

    println!("Julia was successfully installed on your system.");

    if install_choices.modifypath {
        println!();
        println!("Depending on which shell you are using, run one of the following");
        println!(
            "commands to reload the {} environment variable:",
            style("PATH").bold()
        );
        println!();
        for p in &install_choices.modifypath_files {
            println!("  . {}", p.to_string_lossy());
        }
        println!();
    }

    Ok(())
}

#[cfg(not(feature = "selfupdate"))]
pub fn main() -> Result<()> {
    panic!("This should never run.");
}
