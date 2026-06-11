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
