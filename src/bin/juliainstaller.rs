use anyhow::Result;
use clap::Parser;


#[cfg(feature = "selfupdate")]
fn run_individual_config_wizard(    
    new_backgroundselfupdate: i64,
    new_startupselfupdate: i64,
    new_symlinks: bool,
    new_modifypath: bool,
    new_install_location: &str) -> Result<Option<(i64,i64,bool,bool,std::path::PathBuf)>> {

    use dialoguer::{Confirm, Input};
    use std::path::PathBuf;

    let new_install_location = Input::new()
        .with_prompt("Enter the folder where you want to install Juliaup")
        .validate_with(|input: &String| -> Result<(), &str> {
            match input.parse::<PathBuf>() {
                Ok(_) => Ok(()),
                Err(_) => Err("Not a valid input")
            }
        })
        .with_initial_text(new_install_location.to_string())
        .interact_text()?
        .parse::<PathBuf>().unwrap();

    let new_modifypath = match Confirm::new()
        .with_prompt("Do you want to add the Julia binaries to your PATH by manipulating various shell startup scripts?")
        .default(new_modifypath)
        .interact_opt()? {
            Some(value) => value,
            None => return Ok(None)
        };

    let new_symlinks = match Confirm::new()
        .with_prompt("Do you want to add channel specific symlinks?")
        .default(new_symlinks)
        .interact_opt()? {
            Some(value) => value,
            None => return Ok(None)
        };

    let new_startupselfupdate = Input::new()
        .with_prompt("Enter minutes between check for new version at julia startup, use 0 to disable")
        .validate_with(|input: &String| -> Result<(), &str> {
            match input.parse::<i64>() {
                Ok(val) => if val>=0 {Ok(())} else {Err("Not a valid input")},
                Err(_) => Err("Not a valid input")
            }
        })
        .default(new_startupselfupdate.to_string())
        .interact_text()?
        .parse::<i64>().unwrap();

    let new_backgroundselfupdate = Input::new()
        .with_prompt("Enter minutes between check for new version by a background task, use 0 to disable")
        .validate_with(|input: &String| -> Result<(), &str> {
            match input.parse::<i64>() {
                Ok(val) => if val>=0 {Ok(())} else {Err("Not a valid input")},
                Err(_) => Err("Not a valid input")
            }
        })
        .default(new_backgroundselfupdate.to_string())
        .interact_text()?
        .parse::<i64>().unwrap();

    Ok(Some((
        new_backgroundselfupdate,
        new_startupselfupdate,
        new_symlinks,
        new_modifypath,
        new_install_location,
     )))
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
#[clap(name="Juliainstaller", version)]
/// The Julia Installer
struct Juliainstaller {
    /// Juliaup channel
    #[clap(long, default_value = "release")]
    juliaupchannel: String,
}

#[cfg(feature = "selfupdate")]
pub fn main() -> Result<()> {
    use std::io::Seek;
    use dialoguer::Confirm;
    use anyhow::{anyhow, Context};
    use juliaup::{get_juliaup_target, utils::get_juliaserver_base_url, get_own_version, operations::download_extract_sans_parent, config_file::{JuliaupSelfConfig}, command_initial_setup_from_launcher::run_command_initial_setup_from_launcher, command_selfchannel::run_command_selfchannel, global_paths::get_paths};

    human_panic::setup_panic!(human_panic::Metadata {
        name: "Juliainstaller".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        authors: "".into(),
        homepage: "https://github.com/JuliaLang/juliaup".into(),
    });

    let env = env_logger::Env::new().filter("JULIAUP_LOG").write_style("JULIAUP_LOG_STYLE");
    env_logger::init_from_env(env);

    info!("Parsing command line arguments.");
    let args = Juliainstaller::parse();

    let mut paths = get_paths()
        .with_context(|| "Trying to load all global paths.")?;

    use console::Style;
    use dialoguer::{theme::ColorfulTheme, Select};

    use juliaup::{command_config_backgroundselfupdate::run_command_config_backgroundselfupdate, command_config_startupselfupdate::run_command_config_startupselfupdate, command_config_modifypath::run_command_config_modifypath, command_config_symlinks::run_command_config_symlinks};
    use log::{info, trace, debug};

    let theme = ColorfulTheme {
        values_style: Style::new().yellow().dim(),
        ..ColorfulTheme::default()
    };

    eprintln!("Welcome to the Julia setup wizard");
    eprintln!("");
    eprintln!("You can abort at any time with Esc or q, in which case no changes will be made to your system.");
    eprintln!("");

    if is_juliaup_installed() {
        eprintln!("It seems that Juliaup is already installed on this system. Please remove the previous installation of Juliaup before you try to install a new version.");

        return Ok(())
    }

    if paths.juliaupconfig.exists() {
        eprintln!("While Juliaup does not seem to be installed on this system, there is a configuration file present from a previous installation.");

        let continue_with_setup = Confirm::new()
            .with_prompt("Do you want to continue with the installation and overwrite the existing Juliaup configuration file?")
            .default(true)
            .interact_opt()?;

        if !continue_with_setup.unwrap_or(false) {
            return Ok(())
        }
        eprintln!("");
    }

    // First step is to figure out what defaults we would use on this system.
    let default_backgroundselfupdate = 0;
    let default_startupselfupdate = 1440;
    let default_symlinks = false;
    let default_modifypath = true; // TODO Later only set this if `~/.local/bin` is not on the `PATH`
    let default_install_location = dirs::home_dir()
        .ok_or(anyhow!("Could not determine the path of the user home directory."))?
        .join(".juliaup");

    let mut new_backgroundselfupdate = default_backgroundselfupdate;
    let mut new_startupselfupdate = default_startupselfupdate;
    let mut new_symlinks = default_symlinks;
    let mut new_modifypath = default_modifypath;
    let mut new_install_location = default_install_location;

    debug!("Next running the prompt for default choices");

    let choice = Select::with_theme(&theme)
            .with_prompt("Do you want to install with all default configuration values?")
            .default(0)
            .item("Yes, install with defaults")
            .item("No, let me chose custom install options")
            .interact_opt()?;

    trace!("choice is {:?}", choice);

    eprintln!("");

    if choice.is_none() {
        debug!("Exiting because abort was chosen");
        return Ok(())
    }
    else if choice.unwrap() == 1 {
        debug!("Next running the individual config wizard");

        let choice = run_individual_config_wizard(
            new_backgroundselfupdate,
            new_startupselfupdate,
            new_symlinks,
            new_modifypath,
            &new_install_location.to_string_lossy().to_string(),
        )?;

        trace!("choice is {:?}", choice);

        match choice {
            Some(value) => {
                new_backgroundselfupdate = value.0;
                new_startupselfupdate = value.1;
                new_symlinks = value.2;
                new_modifypath = value.3;
                new_install_location= value.4;
            },
            None => {
                debug!("Exiting because abort was chosen");
                return Ok(())
            }
        }
    }

    let juliaupselfbin = new_install_location.join("bin");

    trace!("Set juliaupselfbin to `{:?}`", juliaupselfbin);

    eprintln!("Now installing Juliaup");

    std::fs::create_dir_all(&juliaupselfbin)
        .with_context(|| "Failed to create install folder for Juliaup.")?;

    let juliaup_target = get_juliaup_target();

    let juliaupserver_base = get_juliaserver_base_url()
        .with_context(|| "Failed to get Juliaup server base URL.")?;

    let version = get_own_version().unwrap();
    // let version = semver::Version::parse("1.5.7").unwrap();

    let download_url_path = format!("juliaup/bin/juliaup-{}-{}.tar.gz", version, juliaup_target);

    let new_juliaup_url = juliaupserver_base.join(&download_url_path)
        .with_context(|| format!("Failed to construct a valid url from '{}' and '{}'.", juliaupserver_base, download_url_path))?;

    download_extract_sans_parent(&new_juliaup_url.to_string(), &juliaupselfbin, 0)?;

    {
        let new_selfconfig_data = JuliaupSelfConfig {
            background_selfupdate_interval: None,
            startup_selfupdate_interval: None,
            modify_path: false,
            juliaup_channel: None,
            last_selfupdate: None,
        };

        let self_config_path = new_install_location.join("juliaupself.json");

        let mut self_file = std::fs::OpenOptions::new().create(true).write(true).open(&self_config_path)
            .with_context(|| "Failed to open juliaup config file.")?;

        self_file.rewind()
            .with_context(|| "Failed to rewind self config file for write.")?;

        self_file.set_len(0)
            .with_context(|| "Failed to set len to 0 for self config file before writing new content.")?;

        serde_json::to_writer_pretty(&self_file, &new_selfconfig_data)
            .with_context(|| format!("Failed to write self configuration file."))?;

        self_file.sync_all()
            .with_context(|| "Failed to write config data to disc.")?;     
            
        paths.juliaupselfbin = juliaupselfbin.clone();
        paths.juliaupselfconfig = self_config_path.clone();
    }

    run_command_config_backgroundselfupdate(Some(new_backgroundselfupdate), true, &paths).unwrap();
    run_command_config_startupselfupdate(Some(new_startupselfupdate), true, &paths).unwrap();
    run_command_config_modifypath(Some(new_modifypath), true, &paths).unwrap();
    run_command_config_symlinks(Some(new_symlinks), true, &paths).unwrap();
    run_command_selfchannel(args.juliaupchannel, &paths).unwrap();

    run_command_initial_setup_from_launcher(&paths)?;

    let symlink_path = juliaupselfbin.join("julia");

    std::os::unix::fs::symlink(juliaupselfbin.join("julialauncher"), &symlink_path)
        .with_context(|| format!("failed to create symlink `{}`.", symlink_path.to_string_lossy()))?;

    eprintln!("Julia was successfully installed on your system.");

    if new_modifypath {
        eprintln!("");
        eprintln!("Run `. ~/.bashrc` to reload $PATH variable.")
    }

    Ok(())
}

#[cfg(not(feature = "selfupdate"))]
pub fn main() -> Result<()> {
    panic!("This should never run.");
}
