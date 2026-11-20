use crate::cli;
use anyhow::{Context, Result};
use clap::CommandFactory;
use clap_complete::{generate, Shell};
use cli::CompletionShell;
use std::io::{self, Write};
use std::path::Path;

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

fn generate_julia_launcher_completion(shell: &CompletionShell, stdout: &mut impl Write) {
    let script = match shell {
        CompletionShell::Bash => {
            r#"
_julia_channel_completions() {
    if [[ "${COMP_WORDS[COMP_CWORD]}" == +* ]]; then
        local prefix="${COMP_WORDS[COMP_CWORD]#+}"
        local channels
        channels=$(juliaup _list-channels 2>/dev/null)
        COMPREPLY=($(compgen -P '+' -W "$channels" -- "$prefix"))
    fi
}
complete -o default -F _julia_channel_completions julia
"#
        }
        CompletionShell::Zsh => {
            r#"
_julia_channel() {
    if [[ "$PREFIX" == +* ]]; then
        local -a channels
        channels=(${(f)"$(juliaup _list-channels 2>/dev/null)"})
        IPREFIX="${IPREFIX}+"
        PREFIX="${PREFIX#+}"
        compadd -a channels
        return
    fi
    _default
}
compdef _julia_channel julia
"#
        }
        CompletionShell::Fish => {
            r#"
complete -c julia -n '__fish_is_first_token' -a '(juliaup _list-channels 2>/dev/null | string replace -r "^" "+")' -d 'Julia channel'
"#
        }
        CompletionShell::PowerShell => {
            r#"
Register-ArgumentCompleter -Native -CommandName julia -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    if ($wordToComplete -like '+*') {
        $prefix = $wordToComplete.Substring(1)
        $channels = juliaup _list-channels 2>$null
        $channels | Where-Object { $_ -like "$prefix*" } | ForEach-Object {
            [System.Management.Automation.CompletionResult]::new("+$_", "+$_", 'ParameterValue', $_)
        }
    }
}
"#
        }
        CompletionShell::Elvish => {
            r#"
set edit:completion:arg-completer[julia] = {|@args|
    if (and (> (count $args) 1) (has-prefix $args[1] '+')) {
        var prefix = (str:trim-prefix $args[1] '+')
        juliaup _list-channels 2>/dev/null | each {|ch|
            if (has-prefix $ch $prefix) {
                put '+'$ch
            }
        }
    }
}
"#
        }
        CompletionShell::Nushell => {
            r#"
module julia_completions {
    def julia_channels [] {
        ^juliaup _list-channels | lines | each {|line| $"+($line)"}
    }
    export extern julia [
        channel?: string@julia_channels
        ...rest: string
    ]
}
export use julia_completions *
"#
        }
    };
    let _ = stdout.write_all(script.as_bytes());
}

/// For zsh, ensure `compdef` is available before any `compdef` calls below.
/// If this file is sourced before the user's own `compinit` has run (a common
/// case when the juliaup init block sits near the top of `.zshrc`), initialize
/// the completion system ourselves so registrations don't silently no-op.
fn generate_zsh_compinit_guard(writer: &mut impl Write) {
    let script = r#"if ! (( $+functions[compdef] )); then
    autoload -Uz compinit && compinit
fi
"#;
    let _ = writer.write_all(script.as_bytes());
}

fn generate_completions_to_writer<T: CommandFactory>(
    shell: &CompletionShell,
    app_name: &str,
    writer: &mut impl Write,
) {
    let mut cmd = T::command();
    if matches!(shell, CompletionShell::Zsh) {
        // clap_complete's zsh output starts with a `#compdef <name>` line.
        // zsh's compinit only recognizes a file in $fpath as a completion
        // function if that line is literally the first line of the file, so
        // the compinit guard must be inserted after it rather than before it
        // (see #1580).
        let mut generated = Vec::new();
        generate(Shell::Zsh, &mut cmd, app_name, &mut generated);
        let split_at = generated
            .iter()
            .position(|&b| b == b'\n')
            .map(|i| i + 1)
            .unwrap_or(generated.len());
        let (compdef_line, rest) = generated.split_at(split_at);
        let _ = writer.write_all(compdef_line);
        generate_zsh_compinit_guard(writer);
        let _ = writer.write_all(rest);
    } else {
        match GeneratorType::from(shell.clone()) {
            GeneratorType::Standard(s) => generate(s, &mut cmd, app_name, writer),
            GeneratorType::Nushell => {
                generate(clap_complete_nushell::Nushell, &mut cmd, app_name, writer)
            }
        }
    }
    generate_julia_launcher_completion(shell, writer);
}

/// Write pre-generated completion files to the juliaup completions directory.
/// Called during initial setup and self-update to avoid eval overhead on every shell startup.
pub fn write_completion_files<T: CommandFactory>(juliauphome: &Path, app_name: &str) -> Result<()> {
    let completions_dir = juliauphome.join("completions");
    std::fs::create_dir_all(&completions_dir)
        .with_context(|| "Failed to create completions directory.")?;

    let shells = [
        (CompletionShell::Bash, "bash.sh"),
        (CompletionShell::Zsh, "zsh.zsh"),
        (CompletionShell::Fish, "fish.fish"),
        (CompletionShell::PowerShell, "powershell.ps1"),
        (CompletionShell::Elvish, "elvish.elv"),
        (CompletionShell::Nushell, "nushell.nu"),
    ];

    for (shell, filename) in &shells {
        let path = completions_dir.join(filename);
        let mut buf = Vec::new();
        generate_completions_to_writer::<T>(shell, app_name, &mut buf);
        std::fs::write(&path, &buf)
            .with_context(|| format!("Failed to write completion file: {}", path.display()))?;
    }

    Ok(())
}

/// Generic completion generator that supports both standard shells and nushell
pub fn generate_completion_for_command<T: CommandFactory>(
    shell: CompletionShell,
    app_name: &str,
) -> Result<()> {
    let mut stdout = io::stdout().lock();
    generate_completions_to_writer::<T>(&shell, app_name, &mut stdout);
    Ok(())
}
