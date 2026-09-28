//! Application menu entries for the GUI and the default `julia` channel.
//!
//! On macOS these are `.app` bundles in `~/Applications`, elsewhere they are
//! XDG desktop entries under the user's data directory. Both kinds of link are
//! thin wrappers around the binaries in the Juliaup bin directory, so they keep
//! working after a self-update replaces those binaries.

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
const JULIAUP_ICON: &[u8] = include_bytes!("icons/juliaup.icns");
#[cfg(target_os = "macos")]
const JULIA_ICON: &[u8] = include_bytes!("icons/julia.icns");
#[cfg(not(target_os = "macos"))]
const JULIAUP_ICON: &[u8] = include_bytes!("icons/juliaup.png");
#[cfg(not(target_os = "macos"))]
const JULIA_ICON: &[u8] = include_bytes!("icons/julia.png");

/// The files (or bundle directories) that `create_app_links` writes, so the
/// installer can list them and the uninstaller can remove them.
pub fn app_link_paths() -> Result<Vec<PathBuf>> {
    #[cfg(target_os = "macos")]
    {
        Ok(vec![bundle_path("Juliaup")?, bundle_path("Julia")?])
    }

    #[cfg(not(target_os = "macos"))]
    {
        let dir = app_link_dir()?;
        Ok(vec![
            dir.join(format!("{JULIAUP_DESKTOP_ID}.desktop")),
            dir.join(format!("{JULIA_DESKTOP_ID}.desktop")),
        ])
    }
}

/// Menu launchers do not go through the user's shell, so a depot chosen via
/// the environment has to be baked into them or they would manage a different
/// one. Callers persist this so a later refresh from a bare environment (the
/// background self-update runs from cron) writes the same launchers again.
pub fn depot_from_env() -> Option<String> {
    std::env::var("JULIAUP_DEPOT_PATH")
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

pub fn create_app_links(bin_path: &Path, depot: Option<&str>) -> Result<()> {
    let juliaupgui = bin_path.join("juliaupgui");
    let julia = bin_path.join("julia");

    create_app_link(
        "Juliaup",
        "Install and manage Julia versions",
        &juliaupgui,
        false,
        JULIAUP_ICON,
        depot,
    )?;
    create_app_link(
        "Julia",
        "The Julia programming language",
        &julia,
        true,
        JULIA_ICON,
        depot,
    )?;

    Ok(())
}

pub fn remove_app_links() -> Result<()> {
    for path in app_link_paths()? {
        if !path.exists() || !is_ours(&path) {
            continue;
        }
        let res = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        res.with_context(|| format!("Failed to remove `{}`.", path.display()))?;
    }

    #[cfg(not(target_os = "macos"))]
    remove_icons()?;

    Ok(())
}

// ── macOS ─────────────────────────────────────────────────────────────────────

/// Bundles are created in `~/Applications`, which needs no administrator
/// rights, but the user may move them to `/Applications` afterwards.
#[cfg(target_os = "macos")]
pub const SYSTEM_APPLICATIONS_DIR: &str = "/Applications";

#[cfg(target_os = "macos")]
fn app_link_dir() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .ok_or_else(|| anyhow!("Could not determine the path of the user home directory."))?
        .join("Applications"))
}

/// Where the bundle lives if the user moved it to `/Applications`, otherwise
/// where it is or would be created in `~/Applications`. Refresh and removal
/// follow it there; a same-named bundle in `/Applications` that is not ours
/// (the official Julia app, say) is ignored.
#[cfg(target_os = "macos")]
fn bundle_path(name: &str) -> Result<PathBuf> {
    let moved = Path::new(SYSTEM_APPLICATIONS_DIR).join(format!("{name}.app"));
    if is_ours(&moved) {
        return Ok(moved);
    }
    Ok(app_link_dir()?.join(format!("{name}.app")))
}

/// Write a minimal `.app` bundle whose executable is a shell script that
/// hands over to `target`. Terminal programs are opened through Terminal.app
/// with `open -a`, which needs no automation permissions.
#[cfg(target_os = "macos")]
fn create_app_link(
    name: &str,
    _comment: &str,
    target: &Path,
    terminal: bool,
    icon: &[u8],
    depot: Option<&str>,
) -> Result<()> {
    let bundle = bundle_path(name)?;
    let dir = bundle.parent().unwrap().to_path_buf();
    // Never touch a bundle of the same name that something else installed.
    if bundle.exists() && !is_ours(&bundle) {
        anyhow::bail!(
            "`{}` already exists and was not created by Juliaup.",
            bundle.display()
        );
    }

    // Assemble in a staging directory and swap it in at the end, so an
    // interrupted run never leaves a half-written bundle that `is_ours`
    // would refuse to replace or remove.
    let staging = dir.join(format!(".{name}.app.partial"));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .with_context(|| format!("Failed to remove `{}`.", staging.display()))?;
    }
    let contents = staging.join("Contents");
    let macos_dir = contents.join("MacOS");
    let resources = contents.join("Resources");
    std::fs::create_dir_all(&macos_dir)
        .with_context(|| format!("Failed to create `{}`.", macos_dir.display()))?;
    std::fs::create_dir_all(&resources)
        .with_context(|| format!("Failed to create `{}`.", resources.display()))?;

    let export = match depot {
        Some(d) => format!("export JULIAUP_DEPOT_PATH={}\n", sh_quote(d)),
        None => String::new(),
    };
    let target_sh = sh_quote(&target.to_string_lossy());
    let launcher = format!("#!/bin/sh\n{export}exec {target_sh} \"$@\"\n");
    if terminal {
        // Terminal.app runs whatever executable file it is handed, but does
        // not inherit our environment, so it gets a script rather than the
        // binary itself. Naming it `julia` keeps that as the window title.
        // The inner script is located relative to the outer one, so the
        // bundle keeps working wherever the user moves it.
        let inner = name.to_lowercase();
        write_executable(&resources.join(&inner), &launcher)?;
        write_executable(
            &macos_dir.join(name),
            &format!(
                "#!/bin/sh\nexec open -a Terminal \"$(dirname \"$0\")/../Resources/{inner}\"\n"
            ),
        )?;
    } else {
        write_executable(&macos_dir.join(name), &launcher)?;
    }

    let icon_path = resources.join(format!("{name}.icns"));
    std::fs::write(&icon_path, icon)
        .with_context(|| format!("Failed to write `{}`.", icon_path.display()))?;

    let plist_path = contents.join("Info.plist");
    std::fs::write(&plist_path, info_plist(name, terminal))
        .with_context(|| format!("Failed to write `{}`.", plist_path.display()))?;

    if bundle.exists() {
        std::fs::remove_dir_all(&bundle)
            .with_context(|| format!("Failed to remove `{}`.", bundle.display()))?;
    }
    std::fs::rename(&staging, &bundle).with_context(|| {
        format!(
            "Failed to move `{}` to `{}`.",
            staging.display(),
            bundle.display()
        )
    })?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn info_plist(name: &str, terminal: bool) -> String {
    let identifier = if terminal {
        "org.julialang.juliaup.julia"
    } else {
        "org.julialang.juliaup"
    };
    let version = env!("CARGO_PKG_VERSION");
    // A terminal launcher only runs `open` and exits, so keep it off the Dock
    // rather than bouncing an icon for a fraction of a second.
    let ui_element = if terminal {
        "\t<key>LSUIElement</key>\n\t<true/>\n"
    } else {
        ""
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>{name}</string>
	<key>CFBundleDisplayName</key>
	<string>{name}</string>
	<key>CFBundleIdentifier</key>
	<string>{identifier}</string>
	<key>CFBundleExecutable</key>
	<string>{name}</string>
	<key>CFBundleIconFile</key>
	<string>{name}.icns</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>NSHighResolutionCapable</key>
	<true/>
{ui_element}</dict>
</plist>
"#
    )
}

#[cfg(target_os = "macos")]
fn write_executable(path: &Path, content: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, content)
        .with_context(|| format!("Failed to write `{}`.", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("Failed to chmod `{}`.", path.display()))
}

/// A bundle is ours if its Info.plist carries one of our bundle identifiers.
#[cfg(target_os = "macos")]
fn is_ours(bundle: &Path) -> bool {
    std::fs::read_to_string(bundle.join("Contents").join("Info.plist"))
        .map(|plist| {
            plist.contains("<string>org.julialang.juliaup</string>")
                || plist.contains("<string>org.julialang.juliaup.julia</string>")
        })
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ── Linux, FreeBSD and other XDG desktops ─────────────────────────────────────

#[cfg(not(target_os = "macos"))]
const JULIAUP_DESKTOP_ID: &str = "org.julialang.juliaup";
#[cfg(not(target_os = "macos"))]
const JULIA_DESKTOP_ID: &str = "org.julialang.juliaup.julia";

#[cfg(not(target_os = "macos"))]
fn data_dir() -> Result<PathBuf> {
    dirs::data_dir().ok_or_else(|| anyhow!("Could not determine the user data directory."))
}

#[cfg(not(target_os = "macos"))]
fn app_link_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("applications"))
}

#[cfg(not(target_os = "macos"))]
fn icon_dir() -> Result<PathBuf> {
    Ok(data_dir()?
        .join("icons")
        .join("hicolor")
        .join("256x256")
        .join("apps"))
}

/// The desktop ids are reverse-DNS names nobody else uses, so a file with one
/// of those names is ours by construction.
#[cfg(not(target_os = "macos"))]
fn is_ours(_path: &Path) -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
fn create_app_link(
    name: &str,
    comment: &str,
    target: &Path,
    terminal: bool,
    icon: &[u8],
    depot: Option<&str>,
) -> Result<()> {
    let id = if terminal {
        JULIA_DESKTOP_ID
    } else {
        JULIAUP_DESKTOP_ID
    };

    let icon_dir = icon_dir()?;
    std::fs::create_dir_all(&icon_dir)
        .with_context(|| format!("Failed to create `{}`.", icon_dir.display()))?;
    let icon_path = icon_dir.join(format!("{id}.png"));
    std::fs::write(&icon_path, icon)
        .with_context(|| format!("Failed to write `{}`.", icon_path.display()))?;

    let app_dir = app_link_dir()?;
    std::fs::create_dir_all(&app_dir)
        .with_context(|| format!("Failed to create `{}`.", app_dir.display()))?;
    let desktop_path = app_dir.join(format!("{id}.desktop"));
    let exec = match depot {
        Some(d) => format!(
            "env JULIAUP_DEPOT_PATH={} {}",
            desktop_exec_quote(d),
            desktop_exec_quote(&target.to_string_lossy())
        ),
        None => desktop_exec_quote(&target.to_string_lossy()),
    };
    std::fs::write(
        &desktop_path,
        desktop_entry(name, comment, id, &exec, terminal),
    )
    .with_context(|| format!("Failed to write `{}`.", desktop_path.display()))?;

    // Menus pick up new entries on their own eventually; this just makes it
    // immediate where the tool exists.
    let _ = std::process::Command::new("update-desktop-database")
        .arg(&app_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn desktop_entry(name: &str, comment: &str, id: &str, exec: &str, terminal: bool) -> String {
    let (generic, categories, keywords, extra) = if terminal {
        (
            "Julia programming language",
            "Development;Science;",
            "julia;",
            "",
        )
    } else {
        (
            "Julia version manager",
            "Development;",
            "julia;juliaup;",
            "StartupWMClass=juliaup\n",
        )
    };
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={name}\n\
         GenericName={generic}\n\
         Comment={comment}\n\
         Exec={exec}\n\
         Icon={id}\n\
         Terminal={terminal}\n\
         Categories={categories}\n\
         Keywords={keywords}\n\
         {extra}"
    )
}

/// Quote one argument for the `Exec` key of a desktop entry. The spec applies
/// three layers: inside double quotes `"`, `` ` ``, `$` and `\` are
/// backslash-escaped; the whole value then goes through string escaping, which
/// doubles every backslash again; and `%` introduces field codes, so a literal
/// one is `%%`.
#[cfg(not(target_os = "macos"))]
fn desktop_exec_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' | '`' | '$' => {
                out.push_str("\\\\");
                out.push(c);
            }
            '\\' => out.push_str("\\\\\\\\"),
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(not(target_os = "macos"))]
fn remove_icons() -> Result<()> {
    let icon_dir = icon_dir()?;
    for id in [JULIAUP_DESKTOP_ID, JULIA_DESKTOP_ID] {
        let path = icon_dir.join(format!("{id}.png"));
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("Failed to remove `{}`.", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn sh_quote_escapes_single_quotes() {
        assert_eq!(sh_quote("/a b/c"), "'/a b/c'");
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn info_plist_marks_terminal_launcher_as_ui_element() {
        assert!(info_plist("Julia", true).contains("LSUIElement"));
        assert!(!info_plist("Juliaup", false).contains("LSUIElement"));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn desktop_exec_quote_escapes_reserved_characters() {
        assert_eq!(desktop_exec_quote("/a b/c"), "\"/a b/c\"");
        // Quote-layer escapes are themselves doubled by the string-escape layer.
        assert_eq!(desktop_exec_quote("a\"$`"), r#""a\\"\\$\\`""#);
        assert_eq!(desktop_exec_quote("a\\b"), r#""a\\\\b""#);
        assert_eq!(desktop_exec_quote("100%"), "\"100%%\"");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn desktop_entry_for_julia_runs_in_terminal() {
        let entry = desktop_entry("Julia", "c", JULIA_DESKTOP_ID, "\"/x/bin/julia\"", true);
        assert!(entry.starts_with("[Desktop Entry]\n"));
        assert!(entry.contains("Exec=\"/x/bin/julia\"\n"));
        assert!(entry.contains("Terminal=true\n"));
        assert!(!entry.contains("StartupWMClass"));
    }
}
