use crate::cli;
use anyhow::bail;
use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;
use cli::Juliaup;
use std::io;
use std::str::FromStr;

pub fn run_command_completions(shell: &str) -> Result<()> {
    if let Ok(shell) = Shell::from_str(shell) {
        clap_complete::generate(
            shell,
            &mut Juliaup::command(),
            "juliaup",
            &mut io::stdout().lock(),
        );
    } else {
        bail!("'{}' is not a supported shell.", shell)
    }
    Ok(())
}
