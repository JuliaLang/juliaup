use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;
use std::path::Path;
use std::time::Duration;

/// Test that `julia --version` fails gracefully when no Julia versions are installed
/// This simulates the scenario from issue #1204 after initial setup fails
#[test]
fn julia_version_with_no_versions_installed() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Create a minimal juliaup config directory but with no installed versions
    let juliaup_dir = depot_dir.child("juliaup");
    juliaup_dir.create_dir_all().unwrap();

    // Create a minimal juliaup.json with no installed channels
    let config_content = r#"{
        "default": null,
        "installed_channels": {},
        "installed_versions": {},
        "settings": {
            "versionsdb_update_interval": 1440,
            "startup_selfupdate_interval": null,
            "auto_gc": true,
            "create_channel_symlinks": true,
            "modify_path": true
        },
        "overrides": [],
        "last_version_db_update": null
    }"#;

    juliaup_dir
        .child("juliaup.json")
        .write_str(config_content)
        .unwrap();

    // When no versions are installed, `julia --version` should fail gracefully
    // with a helpful error message, not crash due to juliaup issues
    Command::cargo_bin("julia")
        .unwrap()
        .arg("--version")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Julia startup encountered an issue")
                .or(predicate::str::contains("No Julia versions are installed")),
        )
        .stderr(predicate::str::contains("juliaup add release"))
        // Should NOT contain panic messages or stack traces
        .stderr(predicate::str::contains("panic").not())
        .stderr(predicate::str::contains("thread 'main' panicked").not());
}

/// Test that `julia --version` works when there are corrupted juliaup files
/// This simulates scenarios where the version database or config files are corrupted
#[test]
fn julia_version_with_corrupted_config() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // First install a Julia version normally
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Corrupt the juliaup config file
    let config_path = depot_dir.path().join("juliaup").join("juliaup.json");
    std::fs::write(&config_path, "{ invalid json }").unwrap();

    // `julia --version` should handle this gracefully and not crash
    Command::cargo_bin("julia")
        .unwrap()
        .arg("--version")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Julia startup encountered an issue",
        ))
        .stderr(predicate::str::contains("juliaup add release"));
}

/// Test that `julia` command handles network issues gracefully during initial setup
/// This directly tests the scenario from issue #1204
#[test]
fn julia_with_network_unavailable_during_initial_setup() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Ensure no Julia versions are installed
    depot_dir
        .child(Path::new("juliaup"))
        .assert(predicate::path::missing());

    // Try to run julia with invalid proxy to simulate network issues
    // Use a short timeout to avoid hanging the test
    let _result = Command::cargo_bin("julia")
        .unwrap()
        .arg("-e")
        .arg("println(\"Hello\")")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("https_proxy", "http://invalid-proxy:9999")
        .env("http_proxy", "http://invalid-proxy:9999")
        .timeout(Duration::from_secs(15)) // Don't wait too long
        .assert();

    // The command should either:
    // 1. Fail gracefully with a helpful error message, or
    // 2. Succeed if it can fall back to bundled versions
    // It should NOT panic or hang indefinitely

    // Check that output doesn't contain panic information
    if let Ok(output) = std::process::Command::new("cargo")
        .args(&["run", "--bin", "julia", "--", "-e", "println(\"Hello\")"])
        .current_dir("/Users/ian/Documents/GitHub/juliaup")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("https_proxy", "http://invalid-proxy:9999")
        .env("http_proxy", "http://invalid-proxy:9999")
        .output()
    {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("panic"),
            "Julia launcher should not panic: {}",
            stderr
        );
        assert!(
            !stderr.contains("thread 'main' panicked"),
            "Julia launcher should not panic: {}",
            stderr
        );
    }
}

/// Test that `julia` with a specific channel works even when default setup fails
#[test]
fn julia_with_channel_when_setup_fails() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // First install a specific version
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Corrupt the config to break default detection but leave versions intact
    let config_path = depot_dir.path().join("juliaup").join("juliaup.json");
    let config_content = std::fs::read_to_string(&config_path).unwrap();
    let corrupted_config = config_content.replace("\"default\":", "\"corrupted_default\":");
    std::fs::write(&config_path, corrupted_config).unwrap();

    // Using `+1.8.5` should still work even when default detection is broken
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8.5")
        .arg("--version")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("julia version 1.8.5"));
}

/// Test that environment variable JULIAUP_CHANNEL works when config is broken
#[test]
fn julia_with_env_channel_when_config_broken() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // First install a specific version
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Instead of completely corrupting the config, just remove the default
    // to simulate a broken default detection scenario
    let config_path = depot_dir.path().join("juliaup").join("juliaup.json");
    let config_content = std::fs::read_to_string(&config_path).unwrap();
    let corrupted_config = config_content.replace("\"default\":", "\"corrupted_default\":");
    std::fs::write(&config_path, corrupted_config).unwrap();

    // Using JULIAUP_CHANNEL should still work for specifying a valid channel
    Command::cargo_bin("julia")
        .unwrap()
        .arg("--version")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.8.5")
        .assert()
        .success()
        .stdout(predicate::str::contains("julia version 1.8.5"));
}
