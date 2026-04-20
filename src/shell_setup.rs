//! Shell-specific PATH and completions setup.
//!
//! Each shell is represented as a struct that implements [`ShellSetup`]. The
//! trait unifies three operations:
//!
//! - **`does_exist`** — heuristic check for whether the shell is present on
//!   this system (used to filter the post-install reload message).
//! - **`write_setup`** — write PATH / completions configuration for this shell.
//! - **`remove_setup`** — undo what `write_setup` did.
//! - **`source_hint`** — human-readable "run this to reload PATH now" string
//!   shown after install.
//!
//! POSIX-family shells (sh, bash, zsh, csh/tcsh) use the juliaup marker-block
//! mechanism to modify existing rc files. Fish uses `~/.config/fish/conf.d/`
//! so that neither `config.fish` nor any other file needs to be touched.
//!
//! Adding a new shell means adding a struct + `impl ShellSetup` — no
//! changes to the call sites in `operations.rs`.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

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

    /// Write PATH / completions setup for this shell.
    fn write_setup(&self, bin_path: &Path, juliauphome: &Path) -> Result<()>;

    /// Remove PATH / completions setup written by `write_setup`.
    fn remove_setup(&self) -> Result<()>;

    /// The command the user should run right now to pick up the new PATH,
    /// without opening a new terminal.  Returns `None` for shells that don't
    /// need an explicit reload (e.g. fish, which uses conf.d/).
    fn source_hint(&self) -> Option<String>;
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// All shells juliaup knows about, in priority order.
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

/// Shells that appear to be present on the current system.
#[cfg(not(windows))]
pub fn active_shells() -> Vec<Box<dyn ShellSetup>> {
    all_shells()
        .into_iter()
        .filter(|s| s.does_exist())
        .collect()
}

// ---------------------------------------------------------------------------
// Shared marker-block helpers (POSIX-family shells)
// ---------------------------------------------------------------------------

pub(crate) const S_MARKER: &[u8] = b"# >>> juliaup initialize >>>";
pub(crate) const E_MARKER: &[u8] = b"# <<< juliaup initialize <<<";
const HEADER: &[u8] = b"\n\n# !! Contents within this block are managed by juliaup !!\n\n";

use bstr::ByteSlice;
use bstr::ByteVec;
use std::io::{Read, Seek, Write};

pub(crate) fn match_markers(buffer: &[u8]) -> Result<Option<(usize, usize)>> {
    let start_marker = buffer.find(S_MARKER);
    let end_marker = buffer.find(E_MARKER);

    let (start_marker, end_marker) = match (start_marker, end_marker) {
        (Some(sidx), Some(eidx)) => {
            if sidx != buffer.rfind(S_MARKER).unwrap() || eidx != buffer.rfind(E_MARKER).unwrap() {
                bail!("Found multiple startup script sections from juliaup.");
            }
            (sidx, eidx)
        }
        (None, None) => return Ok(None),
        (_, None) => bail!("Found an opening marker but no end marker of juliaup section."),
        (None, _) => bail!("Found an opening marker but no end marker of juliaup section."),
    };

    Ok(Some((start_marker, end_marker + E_MARKER.len())))
}

/// Write (or replace) the juliaup marker block in a single rc file.
pub(crate) fn write_marker_block(
    path: &Path,
    bin_path: &Path,
    juliauphome: &Path,
    build_content: impl FnOnce(&str, &str) -> Vec<u8>,
) -> Result<()> {
    let bin_str = bin_path.to_str().context("Non-UTF-8 binary path")?;
    let home_str = juliauphome.to_str().context("Non-UTF-8 juliauphome path")?;

    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("Failed to open file {}.", path.display()))?;

    let mut buffer: Vec<u8> = Vec::new();
    file.read_to_end(&mut buffer)
        .with_context(|| format!("Failed to read file {}.", path.display()))?;

    let existing = match_markers(&buffer)
        .with_context(|| format!("Error searching juliaup section in {}.", path.display()))?;

    let mut block: Vec<u8> = Vec::new();
    block.extend_from_slice(S_MARKER);
    block.extend_from_slice(HEADER);
    block.extend_from_slice(&build_content(bin_str, home_str));
    block.extend_from_slice(b"\n");
    block.extend_from_slice(E_MARKER);

    match existing {
        Some(pos) => buffer.replace_range(pos.0..pos.1, &block),
        None => {
            buffer.extend_from_slice(b"\n");
            buffer.extend_from_slice(&block);
            buffer.extend_from_slice(b"\n");
        }
    }

    file.rewind()
        .with_context(|| format!("Failed to rewind file {}.", path.display()))?;
    file.set_len(0)
        .with_context(|| format!("Failed to truncate file {}.", path.display()))?;
    file.write_all(&buffer)
        .with_context(|| format!("Failed to write file {}.", path.display()))?;
    file.sync_all()
        .with_context(|| format!("Failed to sync file {}.", path.display()))?;

    Ok(())
}

/// Remove the juliaup marker block from a single rc file.
pub(crate) fn remove_marker_block(path: &Path) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("Failed to open file: {}", path.display()))?;

    let mut buffer: Vec<u8> = Vec::new();
    file.read_to_end(&mut buffer)?;

    let existing = match_markers(&buffer)
        .with_context(|| format!("Error searching juliaup section in {}.", path.display()))?;

    if let Some(pos) = existing {
        buffer.replace_range(pos.0..pos.1, "");
        file.rewind().unwrap();
        file.set_len(0).unwrap();
        file.write_all(&buffer).unwrap();
        file.sync_all().unwrap();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// sh (POSIX)
// ---------------------------------------------------------------------------

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

    fn write_setup(&self, bin_path: &Path, juliauphome: &Path) -> Result<()> {
        for rc in self.rcfiles() {
            if rc.exists() {
                write_marker_block(&rc, bin_path, juliauphome, build_sh_block)?;
            }
        }
        Ok(())
    }

    fn remove_setup(&self) -> Result<()> {
        for rc in self.rcfiles() {
            if rc.exists() {
                remove_marker_block(&rc)?;
            }
        }
        Ok(())
    }

    fn source_hint(&self) -> Option<String> {
        self.rcfiles()
            .into_iter()
            .find(|p| p.exists())
            .map(|p| format!(". {}", p.display()))
    }
}

// ---------------------------------------------------------------------------
// bash
// ---------------------------------------------------------------------------

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

    fn write_setup(&self, bin_path: &Path, juliauphome: &Path) -> Result<()> {
        for rc in self.update_rcs() {
            write_marker_block(&rc, bin_path, juliauphome, build_bash_block)?;
        }
        Ok(())
    }

    fn remove_setup(&self) -> Result<()> {
        for rc in self.rcfiles() {
            if rc.exists() {
                remove_marker_block(&rc)?;
            }
        }
        Ok(())
    }

    fn source_hint(&self) -> Option<String> {
        self.update_rcs()
            .into_iter()
            .next()
            .map(|p| format!(". {}", p.display()))
    }
}

impl Bash {
    fn update_rcs(&self) -> Vec<PathBuf> {
        self.rcfiles().into_iter().filter(|p| p.exists()).collect()
    }
}

// ---------------------------------------------------------------------------
// zsh
// ---------------------------------------------------------------------------

pub struct Zsh;

impl ShellSetup for Zsh {
    fn does_exist(&self) -> bool {
        // Always set up on macOS (default shell), otherwise only if .zshrc exists.
        std::env::consts::OS == "macos"
            || dirs::home_dir()
                .map(|h| h.join(".zshrc").exists())
                .unwrap_or(false)
    }

    fn name(&self) -> &'static str {
        "zsh"
    }

    fn rcfiles(&self) -> Vec<PathBuf> {
        let Some(home) = dirs::home_dir() else {
            return vec![];
        };
        vec![home.join(".zshrc")]
    }

    fn write_setup(&self, bin_path: &Path, juliauphome: &Path) -> Result<()> {
        // On macOS always create .zshrc even if it doesn't exist yet.
        for rc in self.rcfiles() {
            if rc.exists() || std::env::consts::OS == "macos" {
                write_marker_block(&rc, bin_path, juliauphome, build_zsh_block)?;
            }
        }
        Ok(())
    }

    fn remove_setup(&self) -> Result<()> {
        for rc in self.rcfiles() {
            if rc.exists() {
                remove_marker_block(&rc)?;
            }
        }
        Ok(())
    }

    fn source_hint(&self) -> Option<String> {
        self.rcfiles()
            .into_iter()
            .next()
            .map(|p| format!(". {}", p.display()))
    }
}

// ---------------------------------------------------------------------------
// csh / tcsh
// ---------------------------------------------------------------------------

pub struct Tcsh;

impl Tcsh {
    pub fn new() -> Self {
        Self
    }
}

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

    fn write_setup(&self, bin_path: &Path, juliauphome: &Path) -> Result<()> {
        for rc in self.rcfiles() {
            if rc.exists() {
                write_marker_block(&rc, bin_path, juliauphome, |bin_str, _home_str| {
                    build_csh_block(bin_str)
                })?;
            }
        }
        Ok(())
    }

    fn remove_setup(&self) -> Result<()> {
        for rc in self.rcfiles() {
            if rc.exists() {
                remove_marker_block(&rc)?;
            }
        }
        Ok(())
    }

    fn source_hint(&self) -> Option<String> {
        self.rcfiles()
            .into_iter()
            .find(|p| p.exists())
            .map(|p| format!("source {}", p.display()))
    }
}

// ---------------------------------------------------------------------------
// fish (non-Windows only)
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
pub struct Fish;

#[cfg(not(windows))]
impl Fish {
    fn confd_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".config")))?;
        Some(base.join("fish").join("conf.d").join("juliaup.fish"))
    }
}

#[cfg(not(windows))]
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

    fn write_setup(&self, bin_path: &Path, juliauphome: &Path) -> Result<()> {
        let Some(confd_path) = Fish::confd_path() else {
            return Ok(());
        };
        let Some(parent) = confd_path.parent() else {
            return Ok(());
        };

        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create fish conf.d directory: {}",
                parent.display()
            )
        })?;

        let bin_str = bin_path
            .to_str()
            .context("Non-UTF-8 binary path for fish setup")?;
        let home_str = juliauphome
            .to_str()
            .context("Non-UTF-8 juliauphome path for fish setup")?;

        let content = include_str!("shell_scripts/env.fish")
            .replace("{bin_path}", bin_str)
            .replace("{juliauphome}", home_str);

        std::fs::write(&confd_path, content).with_context(|| {
            format!(
                "Failed to write fish conf.d file at {}.",
                confd_path.display()
            )
        })
    }

    fn remove_setup(&self) -> Result<()> {
        if let Some(path) = Fish::confd_path() {
            if path.exists() {
                std::fs::remove_file(&path).with_context(|| {
                    format!("Failed to remove fish conf.d file at {}.", path.display())
                })?;
            }
        }
        Ok(())
    }

    /// Fish auto-loads conf.d/ on every new session — no reload needed.
    fn source_hint(&self) -> Option<String> {
        None
    }
}

#[cfg(not(windows))]
fn which_fish() -> bool {
    std::process::Command::new("fish")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Block content builders
// ---------------------------------------------------------------------------

pub fn build_sh_block(bin_str: &str, _home_str: &str) -> Vec<u8> {
    include_str!("shell_scripts/env.sh")
        .replace("{bin_path}", bin_str)
        .into_bytes()
}

pub fn build_bash_block(bin_str: &str, home_str: &str) -> Vec<u8> {
    include_str!("shell_scripts/env.bash")
        .replace("{bin_path}", bin_str)
        .replace("{juliauphome}", home_str)
        .into_bytes()
}

pub fn build_zsh_block(bin_str: &str, home_str: &str) -> Vec<u8> {
    include_str!("shell_scripts/env.zsh")
        .replace("{bin_path}", bin_str)
        .replace("{juliauphome}", home_str)
        .into_bytes()
}

pub fn build_csh_block(bin_str: &str) -> Vec<u8> {
    include_str!("shell_scripts/env.csh")
        .replace("{bin_path}", bin_str)
        .into_bytes()
}
