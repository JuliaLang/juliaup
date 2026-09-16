use assert_cmd::{cargo::cargo_bin_cmd, Command};
use assert_fs::TempDir;
use std::path::{Path, PathBuf};

/// A test environment that provides convenient methods for running juliaup and julia commands
/// with isolated depot directories.
pub struct TestEnv {
    depot_dir: TempDir,
    home_dir: TempDir,
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)] // May not be used in all test configurations
impl TestEnv {
    /// Create a new test environment with an isolated temporary depot directory
    /// and an isolated home directory, so that code paths that modify files in
    /// the user's home (e.g. shell startup scripts during self-update) never
    /// touch the real one.
    pub fn new() -> Self {
        Self {
            depot_dir: TempDir::new().unwrap(),
            home_dir: TempDir::new().unwrap(),
        }
    }

    /// Apply the isolated depot and home directories to a command.
    pub fn apply_env(&self, cmd: &mut Command) {
        cmd.env("JULIA_DEPOT_PATH", self.depot_dir.path());
        cmd.env("JULIAUP_DEPOT_PATH", self.depot_dir.path());
        cmd.env("HOME", self.home_dir.path());
    }

    /// Get a Command for running juliaup with the test environment's depot paths
    pub fn juliaup(&self) -> Command {
        let mut cmd = cargo_bin_cmd!("juliaup");
        self.apply_env(&mut cmd);
        cmd
    }

    /// Get a Command for running julia with the test environment's depot paths
    pub fn julia(&self) -> Command {
        let mut cmd = cargo_bin_cmd!("julia");
        self.apply_env(&mut cmd);
        cmd
    }

    /// Get the isolated home directory path
    pub fn home_path(&self) -> &Path {
        self.home_dir.path()
    }

    /// Get the path to the juliaup config file
    pub fn config_path(&self) -> PathBuf {
        self.depot_dir.path().join("juliaup").join("juliaup.json")
    }

    /// Get the depot directory path
    pub fn depot_path(&self) -> &Path {
        self.depot_dir.path()
    }
}

/// Metadata fixture for the live-binary smoke tests. These tests exercise the
/// catalog format independently of its current contents. The synthetic series
/// channel points at the rolling nightly so it cannot expire.
#[allow(dead_code)]
pub struct NightlyMetadataServer {
    url: String,
    stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Default for NightlyMetadataServer {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl NightlyMetadataServer {
    pub fn new() -> Self {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let arch = juliaup::operations::default_arch().unwrap();
        let platform = juliaup::operations::build_file_platform(&arch).unwrap();
        let os = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(windows) {
            "winnt"
        } else if cfg!(target_os = "freebsd") {
            "freebsd"
        } else {
            "linux"
        };
        let url_arch = if cfg!(windows) {
            arch.clone()
        } else if arch == "x64" {
            "x86_64".into()
        } else if arch == "x86" {
            "i686".into()
        } else {
            arch.clone()
        };
        let artifact = nightly_artifact(&format!("https://julialangnightlies-s3.julialang.org/bin/{os}/{url_arch}/julia-latest-{platform}.tar.gz"));
        let mut variants = Vec::new();
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            let mut variant = nightly_artifact("https://julialang-nogpl.s3.amazonaws.com/bin-nogpl/linuxnogpl/x86_64/julia-latest-linuxnogpl-x86_64.tar.gz");
            variant["variants"] = serde_json::json!(["nogpl"]);
            variants.push(variant);
        }
        let channel = serde_json::json!({"files": [artifact], "variants": variants});
        let payload =
            serde_json::json!({"nightly": channel.clone(), "1.0-nightly": channel}).to_string();
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr());
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let worker = std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                if let Some(request) = server
                    .recv_timeout(std::time::Duration::from_millis(50))
                    .unwrap()
                {
                    let (body, code) = if request.url() == "/bin/nightlies.json" {
                        (payload.as_str(), 200)
                    } else if request.url() == "/juliaup/RELEASECHANNELDBVERSION" {
                        ("0.0.0", 200)
                    } else {
                        ("", 404)
                    };
                    let _ = request
                        .respond(tiny_http::Response::from_string(body).with_status_code(code));
                }
            }
        });
        Self {
            url,
            stopped,
            worker: Some(worker),
        }
    }

    pub fn juliaup(&self, env: &TestEnv) -> Command {
        let mut command = env.juliaup();
        command.env("JULIAUP_SERVER", &self.url);
        command
    }

    pub fn julia(&self, env: &TestEnv) -> Command {
        let mut command = env.julia();
        command.env("JULIAUP_SERVER", &self.url);
        command
    }
}

impl Drop for NightlyMetadataServer {
    fn drop(&mut self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}

/// A raw catalog artifact compatible with this test target.
pub fn nightly_artifact(url: &str) -> serde_json::Value {
    let arch = juliaup::operations::default_arch().unwrap();
    let platform = juliaup::nightlies_db::Platform::current(&arch).unwrap();
    let triplet = match platform.os {
        "linux" => format!("{}-linux-gnu", platform.arch),
        "mac" => format!("{}-apple-darwin14", platform.arch),
        "winnt" => format!("{}-w64-mingw32", platform.arch),
        "freebsd" => format!("{}-unknown-freebsd11.1", platform.arch),
        _ => unreachable!(),
    };
    serde_json::json!({"os": platform.os, "arch": platform.arch, "triplet": triplet,
        "kind": "archive", "extension": "tar.gz", "url": url})
}
