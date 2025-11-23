use anyhow::Result;

pub fn run(
    channel: Option<crate::cli::JuliaupChannel>,
    paths: &crate::global_paths::GlobalPaths,
) -> Result<()> {
    use crate::config_file::{load_mut_config_db, save_config_db};
    use anyhow::Context;

    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`self update` command failed to load configuration data.")?;

    match channel {
        Some(chan) => {
            config_file.self_data.juliaup_channel = Some(chan.to_lowercase().to_string());
            save_config_db(&mut config_file)?;
        }
        None => {
            let channel_name = config_file
                .self_data
                .juliaup_channel
                .expect("juliaup_channel should not be empty.");
            println!("Your juliaup is currently on channel `{}`. Run `juliaup self channel -h` for help on how to set the juliaup channel.", channel_name);
        }
    }

    Ok(())
}
