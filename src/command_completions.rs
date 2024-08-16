use crate::cli;
use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;
use cli::Juliaup;
use std::io;

pub fn run_command_completions(shell: Shell) -> Result<()> {
    clap_complete::generate(
        shell,
        &mut Juliaup::command(),
        "juliaup",
        &mut io::stdout().lock(),
    );
    Ok(())
}
