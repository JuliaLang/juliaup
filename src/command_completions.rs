use crate::cli;
use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{generate, Shell};
use cli::{CompletionShell, Juliaup};
use std::io;

/// Type of completion generator to use
enum GeneratorType {
    Standard(Shell),
    Nushell,
}

pub fn run_command_completions(shell: CompletionShell) -> Result<()> {
    generate_completion_for_command::<Juliaup>(shell, "juliaup")
}

/// Generate completions for juliapkg using the same shell enum as juliaup
pub fn generate_juliapkg_completions<T: CommandFactory>(shell: CompletionShell) -> Result<()> {
    generate_completion_for_command::<T>(shell, "juliapkg")
}

/// Generic completion generator that supports both standard shells and nushell
pub fn generate_completion_for_command<T: CommandFactory>(
    shell: CompletionShell,
    app_name: &str,
) -> Result<()> {
    let mut cmd = T::command();
    let mut stdout = io::stdout().lock();

    let generator_type = match shell {
        CompletionShell::Bash => GeneratorType::Standard(Shell::Bash),
        CompletionShell::Elvish => GeneratorType::Standard(Shell::Elvish),
        CompletionShell::Fish => GeneratorType::Standard(Shell::Fish),
        CompletionShell::PowerShell => GeneratorType::Standard(Shell::PowerShell),
        CompletionShell::Zsh => GeneratorType::Standard(Shell::Zsh),
        CompletionShell::Nushell => GeneratorType::Nushell,
    };

    match generator_type {
        GeneratorType::Standard(s) => generate(s, &mut cmd, app_name, &mut stdout),
        GeneratorType::Nushell => generate(
            clap_complete_nushell::Nushell,
            &mut cmd,
            app_name,
            &mut stdout,
        ),
    }

    Ok(())
}
