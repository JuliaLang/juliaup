use crate::cli;
use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;
use clap_complete_nushell::Nushell;
use cli::{CompletionShell, Juliaup};
use std::io;

fn shell_to_string(shell: CompletionShell) -> &'static str {
    match shell {
        CompletionShell::Bash => "bash",
        CompletionShell::Elvish => "elvish",
        CompletionShell::Fish => "fish",
        CompletionShell::Nushell => "nushell",
        CompletionShell::PowerShell => "powershell",
        CompletionShell::Zsh => "zsh",
    }
}

pub fn run_command_completions(shell: CompletionShell) -> Result<()> {
    generate_completion_for_command::<Juliaup>(shell_to_string(shell), "juliaup")
}

/// Generic completion generator that supports both standard shells and nushell
pub fn generate_completion_for_command<T: CommandFactory>(
    shell: &str,
    app_name: &str,
) -> Result<()> {
    let mut cmd = T::command();
    
    // Try to parse as standard clap shell first
    if let Ok(clap_shell) = shell.parse::<Shell>() {
        clap_complete::generate(
            clap_shell,
            &mut cmd,
            app_name,
            &mut io::stdout().lock(),
        );
    } else if shell.eq_ignore_ascii_case("nushell") {
        // Handle nushell separately
        clap_complete::generate(
            Nushell,
            &mut cmd,
            app_name,
            &mut io::stdout().lock(),
        );
    } else {
        anyhow::bail!("Unsupported shell: {}", shell);
    }
    
    Ok(())
}
