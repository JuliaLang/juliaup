use assert_cmd::{cargo::cargo_bin_cmd, Command};
use assert_fs::TempDir;
use std::path::{Path, PathBuf};

/// A test environment that provides convenient methods for running juliaup and julia commands
/// with isolated depot directories.
pub struct TestEnv {
    depot_dir: TempDir,
}

#[allow(dead_code)] // May not be used in all test configurations
impl TestEnv {
    /// Create a new test environment with an isolated temporary depot directory
    pub fn new() -> Self {
        Self {
            depot_dir: TempDir::new().unwrap(),
        }
    }

    /// Get a Command for running juliaup with the test environment's depot paths
    pub fn juliaup(&self) -> Command {
        let mut cmd = cargo_bin_cmd!("juliaup");
        cmd.env("JULIA_DEPOT_PATH", self.depot_dir.path());
        cmd.env("JULIAUP_DEPOT_PATH", self.depot_dir.path());
        cmd
    }

    /// Get a Command for running julia with the test environment's depot paths
    pub fn julia(&self) -> Command {
        let mut cmd = cargo_bin_cmd!("julia");
        cmd.env("JULIA_DEPOT_PATH", self.depot_dir.path());
        cmd.env("JULIAUP_DEPOT_PATH", self.depot_dir.path());
        cmd
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
