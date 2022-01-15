use anyhow::Result;
use clap::Parser;


#[cfg(feature = "selfupdate")]
fn run_individual_config_wizard(install_choices: &mut InstallChoices) -> Result<()> {

    use std::path::PathBuf;

    use log::trace;
    use requestty::{Question, prompt};

    trace!("install_location pre inside the prompt function {:?}", install_choices.install_location);

    let question_installation_location = Question::input("new_install_location")
        .message("Enter the folder where you want to install Juliaup")
        // TODO There is a bug here that if we specify a default, and the user just presses Enter
        // we get an empty return value
        // .default(install_choices.install_location.to_string_lossy().clone())
        .validate(|input, _previous_answer| match input.parse::<PathBuf>() {
                Ok(_) => Ok(()),
                Err(_) => Err("Not a valid input".to_owned())
        })
        .build();

    let question_modifypath = Question::confirm("modifypath")
        .message("Do you want to add the Julia binaries to your PATH by manipulating various shell startup scripts?")
        .default(install_choices.modifypath)
        .build();

    let question_symlinks = Question::confirm("symlinks")
        .message("Do you want to add channel specific symlinks?")
        .default(install_choices.symlinks)
        .build();

    let question_startupselfupdate = Question::int("startupselfupdate")
        .message("Enter minutes between check for new version at julia startup, use 0 to disable")
        .validate(|input, _previous| if input>0 {Ok(())} else {Err("Not a valid input".to_owned())})
        .default(install_choices.startupselfupdate)
        .build();

    let question_backgroundselfupdate = Question::int("backgroundselfupdate")
        .message("Enter minutes between check for new version by a background task, use 0 to disable")
        .validate(|input, _previous| if input>0 {Ok(())} else {Err("Not a valid input".to_owned())})
        .default(install_choices.backgroundselfupdate)
        .build();

    let questions = vec![
        question_installation_location,
        question_modifypath,
        question_symlinks,
        question_startupselfupdate,
        question_backgroundselfupdate,
    ];

    let answers = prompt(questions)?;

    trace!("install_location post inside the prompt function {}", answers["new_install_location"].as_string().unwrap());

    install_choices.install_location = answers["new_install_location"].as_string().unwrap().parse::<PathBuf>().unwrap();
    install_choices.modifypath = answers["modifypath"].as_bool().unwrap();
    install_choices.symlinks = answers["symlinks"].as_bool().unwrap();
    install_choices.startupselfupdate = answers["startupselfupdate"].as_int().unwrap();
    install_choices.backgroundselfupdate = answers["backgroundselfupdate"].as_int().unwrap();

    Ok(())
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
    use crossterm::{style::{Stylize, Attribute}};

    println!("Juliaup will be installed into the Juliaup home directory, located at:");
    println!("");
    println!("  {}", install_choices.install_location.to_string_lossy());
    println!("");
    println!("The {}, {} and other commands will be added to", "julia".attribute(Attribute::Bold), "juliaup".attribute(Attribute::Bold));
    println!("Juliaup's bin directory, located at:");
    println!("");
    println!("  {}", install_choices.install_location.join("bin").to_string_lossy());
    println!("");
    println!("This path will then be added to your {} environment variable by", "PATH".attribute(Attribute::Bold));
    println!("modifying the profile files located at:");
    println!("");
    for p in &install_choices.modifypath_files {
        println!("  {}", p.to_string_lossy());
    }
    println!("");

    Ok(())
}

#[cfg(feature = "selfupdate")]
pub fn main() -> Result<()> {
    use std::io::Seek;
    use anyhow::{anyhow, Context};
    use crossterm::style::{Stylize, Attribute};
    use juliaup::{get_juliaup_target, utils::get_juliaserver_base_url, get_own_version, operations::{download_extract_sans_parent, find_shell_scripts_to_be_modified}, config_file::{JuliaupSelfConfig}, command_initial_setup_from_launcher::run_command_initial_setup_from_launcher, command_selfchannel::run_command_selfchannel, global_paths::get_paths};

    human_panic::setup_panic!(human_panic::Metadata {
        name: "Juliainstaller".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        authors: "".into(),
        homepage: "https://github.com/JuliaLang/juliaup".into(),
    });

    let env = env_logger::Env::new().filter("JULIAUP_LOG").write_style("JULIAUP_LOG_STYLE");
    env_logger::init_from_env(env);

    if atty::isnt(atty::Stream::Stdin) {
        return Err(anyhow!("Stdin is not a tty, this scenario is not yet supported."))
    }

    info!("Parsing command line arguments.");
    let args = Juliainstaller::parse();

    let mut paths = get_paths()
        .with_context(|| "Trying to load all global paths.")?;

    use juliaup::{command_config_backgroundselfupdate::run_command_config_backgroundselfupdate, command_config_startupselfupdate::run_command_config_startupselfupdate, command_config_modifypath::run_command_config_modifypath, command_config_symlinks::run_command_config_symlinks};
    use log::{info, trace, debug};
    use requestty::{Question, prompt_one};

    println!("{}", "Welcome to Julia!".attribute(Attribute::Bold));
    println!("");

    if is_juliaup_installed() {
        println!("It seems that Juliaup is already installed on this system. Please remove the previous installation of Juliaup before you try to install a new version.");

        return Ok(())
    }

    println!("This will download and install the official Julia Language distribution");
    println!("and its version manager Juliaup.");
    println!();

    if paths.juliaupconfig.exists() {
        println!("While Juliaup does not seem to be installed on this system, there is a");
        println!("Juliaup configuration file present from a previous installation.");

        let question_continue_with_setup = Question::confirm("overwrite")
            .message("Do you want to continue with the installation and overwrite the existing Juliaup configuration file?")
            .default(true)
            .build();

        if !prompt_one(question_continue_with_setup)?.as_bool().unwrap() {
            return Ok(());
        }

        println!("");
    }

    let mut install_choices = InstallChoices {
        backgroundselfupdate: 0,
        startupselfupdate: 1440,
        symlinks: false,
        modifypath: true,
        install_location: dirs::home_dir()
            .ok_or(anyhow!("Could not determine the path of the user home directory."))?
            .join(".juliaup"),
        modifypath_files: find_shell_scripts_to_be_modified()?,
    };

    print_install_choices(&install_choices)?;

    println!("You can uninstall at any time with {} and these", "juliaup self uninstall".attribute(Attribute::Bold));
    println!("changes will be reverted.");
    println!("");

    debug!("Next running the prompt for default choices");

    let question_default = Question::select("default")
            .message("Do you want to install with these default configuration choices?")
            .choice("Proceed with installation")
            .choice("Customize installation")
            .choice("Cancel installation")
            .default(0)
            .build();

    let answer_default = prompt_one(question_default)?;
    let answer_default = answer_default.as_list_item().unwrap();

    trace!("choice is {:?}", answer_default);

    println!("");

    if answer_default.index == 1 {
        debug!("Next running the individual config wizard");

        loop {
            run_individual_config_wizard(&mut install_choices)?;

            print_install_choices(&install_choices)?;
    
            let question_confirmcustom = Question::select("confirmcustom")
                .message("Do you want to install with these custom configuration choices?")
                .choice("Proceed with installation")
                .choice("Customize installation")
                .choice("Cancel installation")
                .default(0)
                .build();

            trace!("homedir is {:?}", install_choices.install_location);
    
            let answer_confirmcustom = prompt_one(question_confirmcustom)?;
            let answer_confirmcustom = answer_confirmcustom.as_list_item().unwrap();

            if answer_confirmcustom.index == 0 {
                break;
            }
            else if answer_confirmcustom.index == 2 {
                return Ok(());
            }
        }
    }
    else if answer_default.index == 2 {
        return Ok(());
    }

    let juliaupselfbin = install_choices.install_location.join("bin");

    trace!("Set juliaupselfbin to `{:?}`", juliaupselfbin);

    println!("Now installing Juliaup");

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

        let self_config_path = install_choices.install_location.join("juliaupself.json");

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

    run_command_config_backgroundselfupdate(Some(install_choices.backgroundselfupdate), true, &paths).unwrap();
    run_command_config_startupselfupdate(Some(install_choices.startupselfupdate), true, &paths).unwrap();
    run_command_config_modifypath(Some(install_choices.modifypath), true, &paths).unwrap();
    run_command_config_symlinks(Some(install_choices.symlinks), true, &paths).unwrap();
    run_command_selfchannel(args.juliaupchannel, &paths).unwrap();

    run_command_initial_setup_from_launcher(&paths)?;

    let symlink_path = juliaupselfbin.join("julia");

    std::os::unix::fs::symlink(juliaupselfbin.join("julialauncher"), &symlink_path)
        .with_context(|| format!("failed to create symlink `{}`.", symlink_path.to_string_lossy()))?;

    println!("Julia was successfully installed on your system.");

    if install_choices.modifypath {
        println!("");
        println!("Depending on which shell you are using, run one of the following");
        println!("commands to reload the the {} environment variable:", "PATH".attribute(Attribute::Bold));
        println!("");
        for p in &install_choices.modifypath_files {
            println!("  . {}", p.to_string_lossy());
        }
        println!("");
    }

    Ok(())
}

#[cfg(not(feature = "selfupdate"))]
pub fn main() -> Result<()> {
    panic!("This should never run.");
}
