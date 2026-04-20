//! Shell-specific PATH and completions setup.
//!
//! Each shell is represented as a struct that implements [`ShellSetup`]. The
//! trait provides:
//!
//! - **`does_exist`** — heuristic check for whether the shell is present.
//! - **`name`** — display name for post-install messages.
//! - **`rcfiles`** — all rc-files this shell cares about (for cleanup scanning).
//! - **`update_rcs`** — the subset of `rcfiles` that should actually be written
//!   to (default: existing files only).
//! - **`env_script`** — returns the substituted script content to write.
//! - **`source_hint`** — human-readable "run this to reload PATH" string.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

// ---------------------------------------------------------------------------
// Public trait
// ---------------------------------------------------------------------------

pub trait ShellSetup {
    /// Heuristic: does this shell appear to be installed / in use?
    fn does_exist(&self) -> bool;

    /// Display name used in post-install messages.
    fn name(&self) -> &'static str;

    /// All rc-files this shell cares about (used for cleanup scanning).
    fn rcfiles(&self) -> Vec<PathBuf>;

    /// The subset of `rcfiles` that should actually be written to.
    /// Default: rc files that already exist on disk.
    fn update_rcs(&self) -> Vec<PathBuf> {
        self.rcfiles().into_iter().filter(|p| p.exists()).collect()
    }

    /// The raw script template for this shell (placeholders: `{bin_path}`, `{juliauphome}`).
    fn template(&self) -> &'static str;

    /// Returns the fully-substituted script content to write for this shell.
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
pub fn all_shells() -> Vec<Box<dyn ShellSetup>> {
    vec![
        Box::new(Sh),
        Box::new(Bash),
        Box::new(Zsh),
        Box::new(Tcsh),
        Box::new(Fish),
    ]
}

#[cfg(windows)]
pub fn all_shells() -> Vec<Box<dyn ShellSetup>> {
    vec![]
}

/// Shells that appear to be present on the current system.
#[cfg(not(windows))]
pub fn active_shells() -> Vec<Box<dyn ShellSetup>> {
    all_shells()
        .into_iter()
        .filter(|s| s.does_exist())
        .collect()
}

#[cfg(windows)]
pub fn active_shells() -> Vec<Box<dyn ShellSetup>> {
    vec![]
}

pub struct Sh;

impl ShellSetup for Sh {
    fn does_exist(&self) -> bool {
        // .profile is the POSIX baseline; if nothing else exists we still write it.
        true
    }

    fn name(&self) -> &'static str {
        "sh"
    }

    fn rcfiles(&self) -> Vec<PathBuf> {
        let Some(home) = dirs::home_dir() else {
            return vec![];
        };
        vec![home.join(".profile")]
    }

    fn template(&self) -> &'static str {
        include_str!("shell_scripts/env.sh")
    }

    fn source_hint(&self) -> Option<String> {
        self.rcfiles()
            .into_iter()
            .find(|p| p.exists())
            .map(|p| format!(". {}", p.display()))
    }
}

pub struct Bash;

impl ShellSetup for Bash {
    fn does_exist(&self) -> bool {
        !self.update_rcs().is_empty()
    }

    fn name(&self) -> &'static str {
        "bash"
    }

    fn rcfiles(&self) -> Vec<PathBuf> {
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
        self.update_rcs()
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

impl ShellSetup for Zsh {
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

    fn rcfiles(&self) -> Vec<PathBuf> {
        let Some(zdotdir) = Zsh::zdotdir() else {
            return vec![];
        };
        vec![zdotdir.join(".zshenv")]
    }

    fn template(&self) -> &'static str {
        include_str!("shell_scripts/env.zsh")
    }

    fn update_rcs(&self) -> Vec<PathBuf> {
        // Always write on macOS (default shell); elsewhere only if .zshenv exists.
        self.rcfiles()
            .into_iter()
            .filter(|p| p.exists() || std::env::consts::OS == "macos")
            .collect()
    }

    fn source_hint(&self) -> Option<String> {
        self.rcfiles()
            .into_iter()
            .next()
            .map(|p| format!(". {}", p.display()))
    }
}

pub struct Tcsh;

impl ShellSetup for Tcsh {
    fn does_exist(&self) -> bool {
        let Some(home) = dirs::home_dir() else {
            return false;
        };
        home.join(".cshrc").exists() || home.join(".tcshrc").exists()
    }

    fn name(&self) -> &'static str {
        "csh/tcsh"
    }

    fn rcfiles(&self) -> Vec<PathBuf> {
        let Some(home) = dirs::home_dir() else {
            return vec![];
        };
        vec![home.join(".cshrc"), home.join(".tcshrc")]
    }

    fn template(&self) -> &'static str {
        include_str!("shell_scripts/env.csh")
    }

    fn source_hint(&self) -> Option<String> {
        self.rcfiles()
            .into_iter()
            .find(|p| p.exists())
            .map(|p| format!("source {}", p.display()))
    }
}

pub struct Fish;

impl Fish {
    fn confd_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".config")))?;
        Some(base.join("fish").join("conf.d").join("juliaup.fish"))
    }
}

impl ShellSetup for Fish {
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

    fn rcfiles(&self) -> Vec<PathBuf> {
        Fish::confd_path().into_iter().collect()
    }

    fn update_rcs(&self) -> Vec<PathBuf> {
        // Always write the conf.d file regardless of whether it exists yet.
        Fish::confd_path().into_iter().collect()
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
