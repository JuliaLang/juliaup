use crate::cli;
use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;
use cli::CompletionShell;
use std::io;

/// Type of completion generator to use
enum GeneratorType {
    Standard(Shell),
    Nushell,
}

impl From<CompletionShell> for GeneratorType {
    fn from(shell: CompletionShell) -> Self {
        match shell {
            CompletionShell::Bash => GeneratorType::Standard(Shell::Bash),
            CompletionShell::Elvish => GeneratorType::Standard(Shell::Elvish),
            CompletionShell::Fish => GeneratorType::Standard(Shell::Fish),
            CompletionShell::PowerShell => GeneratorType::Standard(Shell::PowerShell),
            CompletionShell::Zsh => GeneratorType::Standard(Shell::Zsh),
            CompletionShell::Nushell => GeneratorType::Nushell,
        }
    }
}

/// Generic completion generator that supports both standard shells and nushell
pub fn generate<T: CommandFactory>(shell: CompletionShell, app_name: &str) -> Result<()> {
    use clap_complete::generate;
    let mut cmd = T::command();
    let mut stdout = io::stdout().lock();

    match GeneratorType::from(shell) {
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
