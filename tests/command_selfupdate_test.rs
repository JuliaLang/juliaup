//! End-to-end self-update integration tests (see issue #805).
//!
//! These exercise the full upgrade scenario against a local mock server, both
//! for the explicit `juliaup self update` command and for the automatic
//! self-update that `julialauncher` triggers in the background at Julia startup.
//!
//! Requires the `selfupdate` feature (the real self-update code path) and a
//! Unix host (the self-update implementation is non-Windows).
#![cfg(all(unix, feature = "selfupdate"))]

mod utils;
use utils::TestEnv;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use flate2::write::GzEncoder;
use flate2::Compression;
use predicates::prelude::*;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tiny_http::{Header, Response, Server};

/// Build a gzip-compressed tar containing `juliaup` and `julialauncher` at the
/// archive root, mirroring the layout of a real juliaup release tarball.
fn build_juliaup_tarball(juliaup: &Path, julialauncher: &Path) -> Vec<u8> {
    fn append_exec<W: Write>(tar: &mut tar::Builder<W>, src: &Path, name: &str) {
        let data = std::fs::read(src).unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o755);
        tar.append_data(&mut header, name, &data[..]).unwrap();
    }

    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    {
        let mut tar = tar::Builder::new(&mut gz);
        append_exec(&mut tar, juliaup, "juliaup");
        append_exec(&mut tar, julialauncher, "julialauncher");
        tar.finish().unwrap();
    }
    gz.finish().unwrap()
}

/// A local HTTP server that mocks the juliaup release endpoints needed by
/// `juliaup self update`.
struct MockServer {
    base_url: String,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl MockServer {
    /// `db_version` is served for the versions-db version check; serving the
    /// bundled value means no versions-db download is triggered. `new_version`
    /// is served as the available juliaup version, and `tarball` is returned for
    /// any juliaup binary download request.
    fn start(db_version: String, new_version: String, tarball: Vec<u8>) -> Self {
        let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
        let port = server.server_addr().to_ip().unwrap().port();
        let base_url = format!("http://127.0.0.1:{}", port);

        let stop = Arc::new(AtomicBool::new(false));
        let handle = {
            let server = Arc::clone(&server);
            let stop = Arc::clone(&stop);
            thread::spawn(move || loop {
                match server.recv_timeout(Duration::from_millis(100)) {
                    Ok(Some(request)) => {
                        let url = request.url().to_string();
                        let response = if url.ends_with("CHANNELDBVERSION") {
                            Response::from_string(db_version.clone())
                        } else if url.ends_with("CHANNELVERSION") {
                            Response::from_string(new_version.clone())
                        } else if url.contains("/juliaup-") && url.ends_with(".tar.gz") {
                            Response::from_data(tarball.clone())
                        } else {
                            // Catch-all (e.g. the nightly etag-support probe):
                            // respond 200 with an etag header.
                            Response::from_string("").with_header(
                                Header::from_bytes(&b"etag"[..], &b"\"mock\""[..]).unwrap(),
                            )
                        };
                        let _ = request.respond(response);
                    }
                    Ok(None) => {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            })
        };

        MockServer {
            base_url,
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// An isolated juliaup "install" laid out so that the self-update code (which
/// derives its paths from the running executable) operates here instead of
/// clobbering the cargo target directory:
///   <install>/bin/juliaup        (running exe)
///   <install>/bin/julialauncher  (the launcher, named `julia` in dev builds)
///   <install>/bin/julia          (symlink -> julialauncher, as the installer makes)
///   <install>/juliaupself.json   (self config)
struct Install {
    dir: assert_fs::TempDir,
    juliaup_exe: PathBuf,
    julialauncher_exe: PathBuf,
    julia_symlink: PathBuf,
}

impl Install {
    fn setup() -> Self {
        let dir = assert_fs::TempDir::new().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();

        let juliaup_exe = bin.join("juliaup");
        let julialauncher_exe = bin.join("julialauncher");
        std::fs::copy(cargo_bin("juliaup"), &juliaup_exe).unwrap();
        std::fs::copy(cargo_bin("julia"), &julialauncher_exe).unwrap();

        // The installer creates this symlink; self-update wipes it (whole-dir
        // swap) and the `_post-update` hook restores it.
        let julia_symlink = bin.join("julia");
        std::os::unix::fs::symlink(&julialauncher_exe, &julia_symlink).unwrap();

        // A minimal self config is required for selfupdate-feature builds to
        // load the configuration at all.
        std::fs::write(dir.path().join("juliaupself.json"), "{}").unwrap();

        Install {
            dir,
            juliaup_exe,
            julialauncher_exe,
            julia_symlink,
        }
    }

    fn self_config_path(&self) -> PathBuf {
        self.dir.path().join("juliaupself.json")
    }

    fn self_update_recorded(&self) -> bool {
        std::fs::read_to_string(self.self_config_path())
            .map(|s| s.contains("LastSelfUpdate"))
            .unwrap_or(false)
    }
}

/// Run the launcher as if from an interactive terminal session. `script` provides
/// a pseudo-TTY (required by `is_interactive`); `-i` makes `-e` count as interactive.
fn run_julia_interactive(
    julia_symlink: &Path,
    env: &TestEnv,
    extra_env: &[(&str, &str)],
    julia_args: &[&str],
) -> assert_cmd::assert::Assert {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("script");
        c.arg("-q").arg("/dev/null").arg(julia_symlink);
        c.args(julia_args);
        c
    } else {
        let inner = std::iter::once(julia_symlink.to_string_lossy().into_owned())
            .chain(julia_args.iter().map(|s| (*s).to_string()))
            .collect::<Vec<_>>()
            .join(" ");
        Command::new("script").args(["-qefc", &inner, "/dev/null"])
    };

    env.apply_env(&mut cmd);
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    cmd.assert()
}

#[test]
fn self_update_end_to_end() {
    let env = TestEnv::new();
    let install = Install::setup();
    let juliaup_exe = &install.juliaup_exe;
    let julia_symlink = &install.julia_symlink;

    let juliaup = |server: Option<&str>| {
        let mut cmd = Command::new(juliaup_exe);
        env.apply_env(&mut cmd);
        if let Some(server) = server {
            cmd.env("JULIAUP_SERVER", server);
            cmd.env("JULIAUP_NIGHTLY_SERVER", server);
        }
        cmd
    };

    // --- 2. Disable automatic self-update (and background versiondb updates) ---
    juliaup(None)
        .args(["config", "startupselfupdateinterval", "0"])
        .assert()
        .success();
    juliaup(None)
        .args(["config", "backgroundselfupdateinterval", "0"])
        .assert()
        .success();
    juliaup(None)
        .args(["config", "versionsdbupdateinterval", "0"])
        .assert()
        .success();

    // --- 3. Set up state against the real server: a channel, a link, an alias ---
    juliaup(None).args(["add", "1.10.10"]).assert().success();
    juliaup(None)
        .args(["default", "1.10.10"])
        .assert()
        .success();
    juliaup(None)
        .args(["link", "custom", "/usr/bin/false"])
        .assert()
        .success();
    juliaup(None)
        .args(["link", "stable", "+1.10.10"])
        .assert()
        .success();

    // --- 4. Mock a newer juliaup as available on a loopback server ---
    let bundled_db_version = juliaup::get_bundled_dbversion().unwrap().to_string();
    let tarball = build_juliaup_tarball(&install.juliaup_exe, &install.julialauncher_exe);
    let server = MockServer::start(bundled_db_version, "999.0.0".to_string(), tarball);

    // --- 5. Self update ---
    juliaup(Some(&server.base_url))
        .args(["self", "update"])
        .assert()
        .success()
        .stderr(
            predicate::str::contains("Updated Juliaup to version 999.0.0")
                .or(predicate::str::contains("Found new version 999.0.0")),
        );

    // --- 6. Verify the installation still works ---
    // The launcher symlink must have been restored by the post-update hook.
    assert!(
        julia_symlink.symlink_metadata().is_ok(),
        "julia launcher symlink should be restored after self-update"
    );

    // juliaup itself runs and the configured state survived the update.
    juliaup(None).args(["status"]).assert().success().stdout(
        predicate::str::contains("1.10.10")
            .and(predicate::str::contains("custom"))
            .and(predicate::str::contains("stable")),
    );

    // The self-update timestamp was recorded.
    assert!(
        install.self_update_recorded(),
        "self config should record LastSelfUpdate, got: {:?}",
        std::fs::read_to_string(install.self_config_path())
    );

    // And Julia actually runs via the restored launcher.
    let mut julia_cmd = Command::new(julia_symlink);
    env.apply_env(&mut julia_cmd);
    julia_cmd
        .args(["-e", "print(VERSION)"])
        .assert()
        .success()
        .stdout("1.10.10");

    drop(server);
}

/// Verifies the automatic self-update that `julialauncher` triggers at Julia
/// startup in interactive (REPL) mode: when `startupselfupdateinterval` has
/// elapsed, running `julia` spawns `juliaup self update` in a background daemon
/// (and then execs into Julia). Non-interactive invocations such as `julia -e`
/// do not trigger this path.
#[test]
fn self_update_auto_triggered_by_launcher() {
    let env = TestEnv::new();
    let install = Install::setup();
    let juliaup_exe = &install.juliaup_exe;

    let juliaup = || {
        let mut cmd = Command::new(juliaup_exe);
        env.apply_env(&mut cmd);
        cmd
    };

    // Disable background versiondb updates, but enable startup self-update so
    // the launcher will auto-trigger it. With no prior self-update recorded,
    // the interval is considered elapsed on the next `julia` invocation.
    juliaup()
        .args(["config", "versionsdbupdateinterval", "0"])
        .assert()
        .success();
    juliaup()
        .args(["config", "startupselfupdateinterval", "1"])
        .assert()
        .success();

    // Install + default a Julia so the launcher has something to exec into.
    juliaup().args(["add", "1.10.10"]).assert().success();
    juliaup().args(["default", "1.10.10"]).assert().success();

    // Mock a newer juliaup as available on a loopback server.
    let bundled_db_version = juliaup::get_bundled_dbversion().unwrap().to_string();
    let tarball = build_juliaup_tarball(&install.juliaup_exe, &install.julialauncher_exe);
    let server = MockServer::start(bundled_db_version, "999.0.0".to_string(), tarball);

    // Precondition: no self-update has happened yet.
    assert!(
        !install.self_update_recorded(),
        "self-update should not have run before launching julia"
    );

    // Run the launcher via the `julia` symlink with a pseudo-TTY. On Unix it
    // forks a background daemon that auto-triggers `juliaup self update`
    // (inheriting the mock server env), then execs into Julia which prints its
    // version. The self-update proceeds asynchronously after `julia` returns.
    run_julia_interactive(
        &install.julia_symlink,
        &env,
        &[
            ("JULIAUP_SERVER", &server.base_url),
            ("JULIAUP_NIGHTLY_SERVER", &server.base_url),
        ],
        &["-i", "-e", "print(VERSION)"],
    )
    .success()
    .stdout("1.10.10");

    // Wait for the detached self-update to finish: it records the timestamp and
    // the `_post-update` hook restores the launcher symlink. Poll for the final
    // stable state to avoid racing the mid-update directory swap.
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        if install.self_update_recorded() && install.julia_symlink.symlink_metadata().is_ok() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "launcher-triggered self-update did not complete within timeout; self config: {:?}",
            std::fs::read_to_string(install.self_config_path())
        );
        thread::sleep(Duration::from_millis(250));
    }

    // Julia still runs via the restored launcher symlink after the auto-update.
    let mut julia_cmd = Command::new(&install.julia_symlink);
    env.apply_env(&mut julia_cmd);
    julia_cmd
        .args(["-e", "print(VERSION)"])
        .assert()
        .success()
        .stdout("1.10.10");

    drop(server);
}

/// Verifies that non-interactive invocations (e.g. `julia -e`) do not trigger
/// background self-update even when the update interval has elapsed.
#[test]
fn self_update_not_triggered_for_non_interactive() {
    let env = TestEnv::new();
    let install = Install::setup();
    let juliaup_exe = &install.juliaup_exe;

    let juliaup = || {
        let mut cmd = Command::new(juliaup_exe);
        env.apply_env(&mut cmd);
        cmd
    };

    juliaup()
        .args(["config", "versionsdbupdateinterval", "0"])
        .assert()
        .success();
    juliaup()
        .args(["config", "startupselfupdateinterval", "1"])
        .assert()
        .success();

    juliaup().args(["add", "1.10.10"]).assert().success();
    juliaup().args(["default", "1.10.10"]).assert().success();

    let bundled_db_version = juliaup::get_bundled_dbversion().unwrap().to_string();
    let tarball = build_juliaup_tarball(&install.juliaup_exe, &install.julialauncher_exe);
    let server = MockServer::start(bundled_db_version, "999.0.0".to_string(), tarball);

    let mut julia_cmd = Command::new(&install.julia_symlink);
    env.apply_env(&mut julia_cmd);
    julia_cmd
        .env("JULIAUP_SERVER", &server.base_url)
        .env("JULIAUP_NIGHTLY_SERVER", &server.base_url)
        .args(["-e", "print(VERSION)"])
        .assert()
        .success()
        .stdout("1.10.10");

    thread::sleep(Duration::from_secs(3));
    assert!(
        !install.self_update_recorded(),
        "self-update should not run for non-interactive `julia -e` invocations"
    );

    drop(server);
}
