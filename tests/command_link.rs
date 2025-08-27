use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn command_link_binary() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // First add a regular channel for testing
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Test linking to a binary file (existing functionality)
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("custom")
        .arg("/usr/bin/false") // Use a binary that exists but won't work as Julia
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Verify the link shows up in status
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("custom").and(predicate::str::contains("Linked to")));
}

#[test]
fn command_link_alias() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

        // First install a Julia version to create an alias to
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Create an alias to the installed version
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("stable")
        .arg("+1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Channel alias `stable` created, pointing to `1.10.10`."));

    // Verify the alias shows up in status
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("stable").and(predicate::str::contains("Alias to `1.10.10`"))
        );
}

#[test]
fn command_link_alias_to_system_channel() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Test creating an alias to a system channel (release)
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("r")
        .arg("+release")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Channel alias `r` created, pointing to `release`."));

    // Verify the alias shows up in status
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("r").and(predicate::str::contains("Alias to `release`")));
}

#[test]
fn command_link_alias_invalid_target() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Test creating an alias to a non-existent channel
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("broken")
        .arg("+nonexistent")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Target channel `nonexistent` is not installed"));
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
        .stderr(predicate::str::contains("Channel name `1.10.10` is already used"));
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
        .stderr(predicate::str::contains("Julia alias (pointing to 'release') 'r' successfully removed."));

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
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+stable")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
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
    Command::cargo_bin("julia")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("1.10.10");
}

#[test]
fn alias_chain() {
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

    // Create first alias
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("stable")
        .arg("+1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Create alias to alias
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("prod")
        .arg("+stable")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Test that the chained alias works
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+prod")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("1.10.10");

    // Verify both aliases show up in status
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("stable")
                .and(predicate::str::contains("Alias to `1.10.10`"))
                .and(predicate::str::contains("prod"))
                .and(predicate::str::contains("Alias to `stable`"))
        );
}

#[test]
fn alias_circular_reference_detection() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Create first alias
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("a")
        .arg("+release")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Create second alias pointing to first
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("b")
        .arg("+a")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Try to create circular reference - should work for creation
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("remove")
        .arg("a")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("a")
        .arg("+b")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // But using the circular alias should fail
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+a")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Channel alias chain too deep"));
}

#[test]
fn alias_deep_chain_limit() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // Create a very deep chain of aliases to test the depth limit
    let alias_names = ["a1", "a2", "a3", "a4", "a5", "a6", "a7", "a8", "a9", "a10", "a11", "a12"];

    // Start with a system channel
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg(alias_names[0])
        .arg("+release")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Create a chain of aliases
    for i in 1..alias_names.len() {
        Command::cargo_bin("juliaup")
            .unwrap()
            .arg("link")
            .arg(alias_names[i])
            .arg(&format!("+{}", alias_names[i-1]))
            .env("JULIA_DEPOT_PATH", depot_dir.path())
            .env("JULIAUP_DEPOT_PATH", depot_dir.path())
            .assert()
            .success();
    }

    // Using the deep alias should fail due to depth limit
    Command::cargo_bin("julia")
        .unwrap()
        .arg(&format!("+{}", alias_names[alias_names.len()-1]))
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Channel alias chain too deep"));
}

#[test]
fn alias_update_resolves_target() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // First install a Julia version to create an alias to
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Create an alias to the installed version
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("r")
        .arg("+1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Update through the alias - should work and update the target
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("update")
        .arg("r")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();
}
