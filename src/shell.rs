//! Shell-specific PATH and completions setup.
//!
//! When juliaup installs, it places `julia` and `juliaup` binaries in
//! `~/.juliaup/bin`. For those commands to be available in the user's shell,
//! that directory needs to be on PATH — and ideally that should survive new
//! terminal sessions without the user having to do anything manually.
//!
//! The challenge is that each shell has its own conventions for which rc files
//! are sourced, when, and in what order. Login shells, interactive shells, and
//! GUI terminals all behave differently across sh, bash, zsh, tcsh, and fish.
//! Rather than trying to pick the "one right file", juliaup writes a small
//! initialisation block into whichever rc files are appropriate for each shell,
//! wrapped in clearly delimited markers so it can be updated or removed cleanly.
//!
//! Each shell is represented as a struct implementing [`UnixShell`]. See the
//! trait documentation for the methods each shell must provide.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub trait UnixShell {
    // Detects if a shell "exists". Users have multiple shells, so an "eager"
    // heuristic should be used, assuming shells exist if any traces do.
    fn does_exist(&self) -> bool;

    // Returns the display name of the shell, used in post-install messages.
    fn name(&self) -> &'static str;

    // Gives all rcfiles of a given shell that Rustup is concerned with.
    // Used primarily in checking rcfiles for cleanup.
    fn all_rcfiles(&self) -> Vec<PathBuf>;

    // The subset of `all_rcfiles` that should actually be written to.
    // Default: rc files that already exist on disk.
    fn rcfiles_to_write(&self) -> Vec<PathBuf> {
        self.all_rcfiles()
            .into_iter()
            .filter(|p| p.exists())
            .collect()
    }

    // The raw script template for this shell (placeholders: `{bin_path}`, `{juliauphome}`).
    fn template(&self) -> &'static str;

    // Writes the relevant env file.
    fn env_script(&self, bin_path: &Path, juliauphome: &Path) -> Result<Vec<u8>> {
        let bin_str = bin_path.to_str().context("Non-UTF-8 binary path")?;
        let home_str = juliauphome.to_str().context("Non-UTF-8 juliauphome path")?;
        Ok(self
            .template()
            .replace("{bin_path}", bin_str)
            .replace("{juliauphome}", home_str)
            .into_bytes())
    }

    /// The command the user should run right now to pick up the new PATH,
    /// without opening a new terminal.  Returns `None` for shells that don't
    /// need an explicit reload (e.g. fish, which uses conf.d/).
    fn source_hint(&self) -> Option<String>;
}

#[cfg(not(windows))]
pub fn all_shells() -> Vec<Box<dyn UnixShell>> {
    vec![
        Box::new(Posix),
        Box::new(Bash),
        Box::new(Zsh),
        Box::new(Tcsh),
        Box::new(Fish),
    ]
}

#[cfg(windows)]
pub fn all_shells() -> Vec<Box<dyn UnixShell>> {
    vec![]
}

/// Shells that appear to be present on the current system.
#[cfg(not(windows))]
pub fn active_shells() -> Vec<Box<dyn UnixShell>> {
    all_shells()
        .into_iter()
        .filter(|s| s.does_exist())
        .collect()
}

#[cfg(windows)]
pub fn active_shells() -> Vec<Box<dyn UnixShell>> {
    vec![]
}

/// Covers POSIX-compatible shells: sh, ash, dash, pdksh — all source `.profile`.
pub struct Posix;

impl UnixShell for Posix {
    fn does_exist(&self) -> bool {
        // .profile is the POSIX baseline; always write to it so any sh-compatible
        // shell picks up the PATH update.
        true
    }

    fn name(&self) -> &'static str {
        "sh/ash/dash/pdksh"
    }

    fn all_rcfiles(&self) -> Vec<PathBuf> {
        let Some(home) = dirs::home_dir() else {
            return vec![];
        };
        vec![home.join(".profile")]
    }

    fn template(&self) -> &'static str {
        include_str!("shell_scripts/env.sh")
    }

    fn source_hint(&self) -> Option<String> {
        self.all_rcfiles()
            .into_iter()
            .find(|p| p.exists())
            .map(|p| format!(". {}", p.display()))
    }
}

pub struct Bash;

impl UnixShell for Bash {
    fn does_exist(&self) -> bool {
        !self.rcfiles_to_write().is_empty()
    }

    fn name(&self) -> &'static str {
        "bash"
    }

    fn all_rcfiles(&self) -> Vec<PathBuf> {
        let Some(home) = dirs::home_dir() else {
            return vec![];
        };
        [".bashrc", ".bash_profile", ".bash_login"]
            .iter()
            .map(|f| home.join(f))
            .collect()
    }

    fn template(&self) -> &'static str {
        include_str!("shell_scripts/env.bash")
    }

    fn source_hint(&self) -> Option<String> {
        self.rcfiles_to_write()
            .into_iter()
            .next()
            .map(|p| format!(". {}", p.display()))
    }
}

pub struct Zsh;

impl Zsh {
    /// Returns the directory zsh reads its dotfiles from: `$ZDOTDIR` if set,
    /// otherwise `$HOME`.
    fn zdotdir() -> Option<PathBuf> {
        if let Ok(dir) = std::env::var("ZDOTDIR") {
            if !dir.is_empty() {
                return Some(PathBuf::from(dir));
            }
        }
        dirs::home_dir()
    }
}

impl UnixShell for Zsh {
    fn does_exist(&self) -> bool {
        // zsh is the default shell on macOS, or the user has a .zshenv already.
        std::env::consts::OS == "macos"
            || Zsh::zdotdir()
                .map(|d| d.join(".zshenv").exists())
                .unwrap_or(false)
    }

    fn name(&self) -> &'static str {
        "zsh"
    }

    fn all_rcfiles(&self) -> Vec<PathBuf> {
        // Return both ZDOTDIR and HOME candidates so cleanup scans both locations,
        // regardless of which was active when juliaup was first installed.
        [Zsh::zdotdir(), dirs::home_dir()]
            .into_iter()
            .flatten()
            .map(|d| d.join(".zshenv"))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    fn template(&self) -> &'static str {
        include_str!("shell_scripts/env.zsh")
    }

    fn rcfiles_to_write(&self) -> Vec<PathBuf> {
        // Always write on macOS (default shell); elsewhere only if .zshenv exists.
        self.all_rcfiles()
            .into_iter()
            .filter(|p| p.exists() || std::env::consts::OS == "macos")
            .collect()
    }

    fn source_hint(&self) -> Option<String> {
        self.all_rcfiles()
            .into_iter()
            .next()
            .map(|p| format!(". {}", p.display()))
    }
}

pub struct Tcsh;

impl UnixShell for Tcsh {
    fn does_exist(&self) -> bool {
        let Some(home) = dirs::home_dir() else {
            return false;
        };
        home.join(".cshrc").exists() || home.join(".tcshrc").exists()
    }

    fn name(&self) -> &'static str {
        "csh/tcsh"
    }

    fn all_rcfiles(&self) -> Vec<PathBuf> {
        let Some(home) = dirs::home_dir() else {
            return vec![];
        };
        vec![home.join(".cshrc"), home.join(".tcshrc")]
    }

    fn template(&self) -> &'static str {
        include_str!("shell_scripts/env.csh")
    }

    fn source_hint(&self) -> Option<String> {
        self.all_rcfiles()
            .into_iter()
            .find(|p| p.exists())
            .map(|p| format!("source {}", p.display()))
    }
}

pub struct Fish;

impl Fish {
    /// Returns all candidate conf.d paths: XDG_CONFIG_HOME-based first, then
    /// the ~/.config fallback. Both are included in all_rcfiles so cleanup
    /// finds the file regardless of which was active at install time.
    fn confd_paths() -> Vec<PathBuf> {
        let xdg = std::env::var_os("XDG_CONFIG_HOME")
            .map(|x| PathBuf::from(x).join("fish/conf.d/juliaup.fish"));
        let home = dirs::home_dir().map(|h| h.join(".config/fish/conf.d/juliaup.fish"));
        xdg.into_iter()
            .chain(home)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// The single path that should be written to: XDG_CONFIG_HOME if set,
    /// otherwise ~/.config.
    fn confd_write_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".config")))?;
        Some(base.join("fish/conf.d/juliaup.fish"))
    }
}

impl UnixShell for Fish {
    fn does_exist(&self) -> bool {
        // fish must either be the running shell or be callable.
        std::env::var("SHELL")
            .map(|s| s.contains("fish"))
            .unwrap_or(false)
            || which_fish()
    }

    fn name(&self) -> &'static str {
        "fish"
    }

    fn all_rcfiles(&self) -> Vec<PathBuf> {
        // > "$XDG_CONFIG_HOME/fish/conf.d" (or "~/.config/fish/conf.d" if that variable is unset) for the user
        // from <https://github.com/fish-shell/fish-shell/issues/3170#issuecomment-228311857>
        Fish::confd_paths()
    }

    fn rcfiles_to_write(&self) -> Vec<PathBuf> {
        Fish::confd_write_path().into_iter().collect()
    }

    fn template(&self) -> &'static str {
        include_str!("shell_scripts/env.fish")
    }

    /// Fish auto-loads conf.d/ on every new session — no reload needed.
    fn source_hint(&self) -> Option<String> {
        None
    }
}

fn which_fish() -> bool {
    std::process::Command::new("fish")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
