use assert_cmd::Command;
use predicates::prelude::*;

fn juliaup_command(depot_dir: &assert_fs::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("juliaup").unwrap();
    cmd.env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path());
    cmd
}

fn julia_command(depot_dir: &assert_fs::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("julia").unwrap();
    cmd.env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path());
    cmd
}

#[test]
fn command_link_binary() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // First add a regular channel for testing
    juliaup_command(&depot_dir)
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    // Test linking to a binary file (existing functionality)
    juliaup_command(&depot_dir)
        .arg("link")
        .arg("custom")
        .arg("/usr/bin/false") // Use a binary that exists but won't work as Julia
        .assert()
        .success();

    // Verify the link shows up in status
    juliaup_command(&depot_dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("custom").and(predicate::str::contains("Linked to")));
}

#[test]
fn command_link_alias() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // First install a Julia version to create an alias to
    juliaup_command(&depot_dir)
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    // Create an alias to the installed version
    juliaup_command(&depot_dir)
        .arg("link")
        .arg("stable")
        .arg("+1.10.10")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Channel alias `stable` created, pointing to `1.10.10`.",
        ));

    // Verify the alias shows up in status
    juliaup_command(&depot_dir)
        .arg("status")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("stable").and(predicate::str::contains("Alias to `1.10.10`")),
        );
}

#[test]
fn command_link_alias_to_system_channel() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Test creating an alias to a system channel (release)
    juliaup_command(&depot_dir)
        .arg("link")
        .arg("r")
        .arg("+release")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Channel alias `r` created, pointing to `release`.",
        ));

    // Verify the alias shows up in status
    juliaup_command(&depot_dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("r").and(predicate::str::contains("Alias to `release`")));
}

#[test]
fn command_link_alias_invalid_target() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Test creating an alias to a non-existent channel
    juliaup_command(&depot_dir)
        .arg("link")
        .arg("broken")
        .arg("+nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Target channel `nonexistent` is not installed",
        ));
}

#[test]
fn command_link_alias_with_args_fails() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Test that creating an alias with extra arguments fails (the argument parser should reject this)
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("alias_with_args")
        .arg("+release")
        .arg("--some-arg")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument"));
}

#[test]
fn command_link_duplicate_channel() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // First add a regular channel
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Try to create an alias with the same name as an existing channel
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("1.10.10")
        .arg("+release")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Channel name `1.10.10` is already used",
        ));
}

#[test]
fn command_remove_alias() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Create an alias
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("r")
        .arg("+release")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Remove the alias
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("remove")
        .arg("r")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Julia alias (pointing to 'release') 'r' successfully removed.",
        ));

    // Verify the alias is gone from status (check for empty list or no mention of the alias)
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Alias to").not());
}

#[test]
fn command_remove_non_existent() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Try to remove a non-existent channel
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("remove")
        .arg("nonexistent")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("'nonexistent' cannot be removed because it is not currently installed. Please run `juliaup list` to see available channels."));
}

#[test]
fn alias_resolution_julia_launcher() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Add a channel first
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Create an alias to it
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("stable")
        .arg("+1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Try to use the alias with julia +alias
    julia_command(&depot_dir)
        .arg("+stable")
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.10.10");
}

#[test]
fn alias_as_default() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Add a channel first
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Create an alias
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("stable")
        .arg("+1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Set the alias as default
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("default")
        .arg("stable")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Test that julia without + uses the alias
    julia_command(&depot_dir)
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.10.10");
}

#[test]
fn alias_to_alias_prevented() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Add a channel first
    juliaup_command(&depot_dir)
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    // Create first alias
    juliaup_command(&depot_dir)
        .arg("link")
        .arg("stable")
        .arg("+1.10.10")
        .assert()
        .success();

    // Try to create alias to alias - should now fail
    juliaup_command(&depot_dir)
        .arg("link")
        .arg("prod")
        .arg("+stable")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Cannot create an alias to another alias `stable`",
        ));
}

// The old alias_circular_reference_detection and alias_deep_chain_limit tests
// are no longer relevant since we now prevent alias-to-alias chains entirely

#[test]
fn alias_update_resolves_target() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // First install a Julia version to create an alias to
    juliaup_command(&depot_dir)
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    // Create an alias to the installed version
    juliaup_command(&depot_dir)
        .arg("link")
        .arg("r")
        .arg("+1.10.10")
        .assert()
        .success();

    // Update through the alias - should work and update the target
    juliaup_command(&depot_dir)
        .arg("update")
        .arg("r")
        .assert()
        .success();
}
