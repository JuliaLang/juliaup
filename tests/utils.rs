use assert_cmd::Command;
use assert_fs::TempDir;

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
        self.command("juliaup")
    }

    /// Get a Command for running julia with the test environment's depot paths
    pub fn julia(&self) -> Command {
        self.command("julia")
    }

    /// Get a Command for running any binary with the test environment's depot paths
    pub fn command(&self, bin: &str) -> Command {
        let mut cmd = Command::cargo_bin(bin).unwrap();
        cmd.env("JULIA_DEPOT_PATH", self.depot_dir.path());
        cmd.env("JULIAUP_DEPOT_PATH", self.depot_dir.path());
        cmd
    }

    /// Get the path to the depot directory for this test environment
    #[allow(dead_code)] // May not be used in all test configurations
    pub fn depot_path(&self) -> &std::path::Path {
        self.depot_dir.path()
    }
}
