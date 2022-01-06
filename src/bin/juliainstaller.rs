use anyhow::Result;
use clap::Parser;
// use dialoguer::{Confirm, Input};


// fn run_individual_config_wizard() -> Result<(i64,i64,bool,bool)> {
//     let new_modifypath = Confirm::new()
//         .with_prompt("Do you want to add the Julia binaries to your PATH by manipulating various shell startup scripts?")
//         .default(true)
//         .interact_opt()?;

//     let new_symlinks = Confirm::new()
//         .with_prompt("Do you want to add channel specific symlinks?")
//         .default(false)
//         .interact_opt()?;

//     let new_startupselfupdate: String = Input::new()
//         .with_prompt("Enter minutes between check for new version at julia startup, use 0 to disable")
//         .validate_with(|input: &String| -> Result<(), &str> {
//             match input.parse::<i64>() {
//                 Ok(val) => if val>=0 {Ok(())} else {Err("Not a valid input")},
//                 Err(_) => Err("Not a valid input")
//             }
//         })
//         .interact()?;

//     let new_backgroundselfupdate: String = Input::new()
//         .with_prompt("Enter minutes between check for new version by a background task, use 0 to disable")
//         .validate_with(|input: &String| -> Result<(), &str> {
//             match input.parse::<i64>() {
//                 Ok(val) => if val>=0 {Ok(())} else {Err("Not a valid input")},
//                 Err(_) => Err("Not a valid input")
//             }
//         })
//         .interact()?;

//     Ok((
//         new_backgroundselfupdate.parse::<i64>().unwrap(),
//         new_startupselfupdate.parse::<i64>().unwrap(),
//         true, true
//         // new_symlinks,
//         // new_modifypath,
//      ))
// }

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
    use anyhow::{anyhow, Context};
    use juliaup::{get_juliaup_target, utils::get_juliaserver_base_url, get_own_version, operations::download_extract_sans_parent, config_file::{load_mut_config_db, save_config_db}, command_initial_setup_from_launcher::run_command_initial_setup_from_launcher, command_selfchannel::run_command_selfchannel};

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

    use console::Style;
    use dialoguer::{theme::ColorfulTheme, Select, Confirm};

    use juliaup::{command_config_backgroundselfupdate::run_command_config_backgroundselfupdate, command_config_startupselfupdate::run_command_config_startupselfupdate, command_config_modifypath::run_command_config_modifypath, command_config_symlinks::run_command_config_symlinks, utils::get_juliaupconfig_path};
    use log::info;

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

    if get_juliaupconfig_path()?.exists() {
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
        .join(".juliaup")
        .join("bin");

    let new_backgroundselfupdate = default_backgroundselfupdate;
    let new_startupselfupdate = default_startupselfupdate;
    let new_symlinks = default_symlinks;
    let new_modifypath = default_modifypath;
    let new_install_location = default_install_location;

    let choice = Select::with_theme(&theme)
            .with_prompt("Do you want to install with all default configuration values?")
            .default(0)
            .item("Yes, install with defaults")
            .item("No, let me chose custom install options")
            .interact_opt()?;

    eprintln!("");

    if choice.is_none() {
        return Ok(())
    }
    // else if choice.unwrap() == 1 {
    //     let choice = run_individual_config_wizard();
    // }

    eprintln!("Now installing Juliaup");

    std::fs::create_dir_all(&new_install_location)
        .with_context(|| "Failed to create install folder for Juliaup.")?;

    let juliaup_target = get_juliaup_target();

    let juliaupserver_base = get_juliaserver_base_url()
        .with_context(|| "Failed to get Juliaup server base URL.")?;

    // let version = get_own_version().unwrap();
    let version = semver::Version::parse("1.5.7").unwrap();

    let download_url_path = format!("juliaup/bin/juliaup-{}-{}.tar.gz", version, juliaup_target);

    let new_juliaup_url = juliaupserver_base.join(&download_url_path)
        .with_context(|| format!("Failed to construct a valid url from '{}' and '{}'.", juliaupserver_base, download_url_path))?;

    download_extract_sans_parent(&new_juliaup_url.to_string(), &new_install_location, 0)?;

    {
        let mut config_file = load_mut_config_db()
            .with_context(|| "`config` command failed to load configuration data.")?;

        config_file.data.self_install_location = Some(new_install_location.to_string_lossy().to_string());

        save_config_db(&mut config_file)
            .with_context(|| "Failed to save configuration file from `config` command.")?;
    }

    run_command_config_backgroundselfupdate(Some(new_backgroundselfupdate)).unwrap();
    run_command_config_startupselfupdate(Some(new_startupselfupdate)).unwrap();
    run_command_config_modifypath(Some(new_modifypath)).unwrap();
    run_command_config_symlinks(Some(new_symlinks)).unwrap();
    run_command_selfchannel(args.juliaupchannel).unwrap();

    run_command_initial_setup_from_launcher()?;

    let symlink_path = new_install_location.join("julia");

    std::os::unix::fs::symlink(new_install_location.join("julialauncher"), &symlink_path)
        .with_context(|| format!("failed to create symlink `{}`.", symlink_path.to_string_lossy()))?;

    Ok(())
}

#[cfg(not(feature = "selfupdate"))]
pub fn main() -> Result<()> {
    panic!("This should never run.");
}
