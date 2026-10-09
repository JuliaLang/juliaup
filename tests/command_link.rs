use predicates::prelude::*;

mod utils;
use utils::TestEnv;

#[test]
fn command_link_binary() {
    let env = TestEnv::new();

    // First add a regular channel for testing
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Test linking to a binary file (existing functionality)
    env.juliaup()
        .arg("link")
        .arg("custom")
        .arg("/usr/bin/false") // Use a binary that exists but won't work as Julia
        .assert()
        .success();

    // Verify the link shows up in status
    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("custom").and(predicate::str::contains("Linked to")));
}

#[test]
fn command_link_alias() {
    let env = TestEnv::new();

    // First install a Julia version to create an alias to
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Create an alias to the installed version
    env.juliaup()
        .arg("link")
        .arg("stable")
        .arg("+1.10.10")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Channel alias `stable` created, pointing to `1.10.10`.",
        ));

    // Verify the alias shows up in status
    env.juliaup().arg("status").assert().success().stdout(
        predicate::str::contains("stable").and(predicate::str::contains("Alias to `1.10.10`")),
    );
}

#[test]
fn command_link_alias_to_system_channel() {
    let env = TestEnv::new();

    // Test creating an alias to a system channel (release)
    env.juliaup()
        .arg("link")
        .arg("r")
        .arg("+release")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Channel alias `r` created, pointing to `release`.",
        ));

    // Verify the alias shows up in status
    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("r").and(predicate::str::contains("Alias to `release`")));
}

#[test]
fn command_link_alias_invalid_target() {
    let env = TestEnv::new();

    // Test creating an alias to a non-existent channel
    env.juliaup()
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
fn command_link_alias_with_args_works() {
    let env = TestEnv::new();

    // Test that creating an alias with extra arguments works and shows them in the output
    env.juliaup()
        .arg("link")
        .arg("alias_with_args")
        .arg("+release")
        .arg("--")
        .arg("--some-arg")
        .assert()
        .success()
        .stderr(predicate::str::contains("args: [\"--some-arg\"]"));
}

#[test]
fn alias_with_args_passes_through() {
    let env = TestEnv::new();

    // First install a Julia version
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Create an alias with args that will be passed to Julia
    env.juliaup()
        .arg("link")
        .arg("julia_with_threads")
        .arg("+1.10.10")
        .arg("--")
        .arg("--threads=4")
        .arg("--startup-file=no")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "args: [\"--threads=4\", \"--startup-file=no\"]",
        ));

    // Test that the args are actually passed through when running Julia
    // Julia with --threads=4 should report 4 threads
    env.julia()
        .arg("+julia_with_threads")
        .arg("-e")
        .arg("println(Threads.nthreads())")
        .assert()
        .success()
        .stdout("4\n");
}

#[test]
fn command_link_duplicate_channel() {
    let env = TestEnv::new();

    // First add a regular channel
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Try to create an alias with the same name as an existing channel
    env.juliaup()
        .arg("link")
        .arg("1.10.10")
        .arg("+release")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Channel name `1.10.10` is already used",
        ));
}

#[test]
fn command_remove_alias() {
    let env = TestEnv::new();

    // Create an alias
    env.juliaup()
        .arg("link")
        .arg("r")
        .arg("+release")
        .assert()
        .success();

    // Remove the alias
    env.juliaup()
        .arg("remove")
        .arg("r")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Julia alias (pointing to 'release') 'r' successfully removed.",
        ));

    // Verify the alias is gone from status (check for empty list or no mention of the alias)
    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Alias to").not());
}

#[test]
fn command_remove_non_existent() {
    let env = TestEnv::new();

    // Try to remove a non-existent channel
    env.juliaup()
        .arg("remove")
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("'nonexistent' cannot be removed because it is not currently installed. Please run `juliaup list` to see available channels."));
}

#[test]
fn alias_resolution_julia_launcher() {
    let env = TestEnv::new();

    // Add a channel first
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Create an alias to it
    env.juliaup()
        .arg("link")
        .arg("stable")
        .arg("+1.10.10")
        .assert()
        .success();

    // Try to use the alias with julia +alias
    env.julia()
        .arg("+stable")
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.10.10");
}

#[test]
fn alias_as_default() {
    let env = TestEnv::new();

    // Add a channel first
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Create an alias
    env.juliaup()
        .arg("link")
        .arg("stable")
        .arg("+1.10.10")
        .assert()
        .success();

    // Set the alias as default
    env.juliaup()
        .arg("default")
        .arg("stable")
        .assert()
        .success();

    // Test that julia without + uses the alias
    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.10.10");
}

#[test]
fn alias_to_alias_prevented() {
    let env = TestEnv::new();

    // Add a channel first
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Create first alias
    env.juliaup()
        .arg("link")
        .arg("stable")
        .arg("+1.10.10")
        .assert()
        .success();

    // Try to create alias to alias - should now fail
    env.juliaup()
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
    let env = TestEnv::new();

    // First install a Julia version to create an alias to
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Create an alias to the installed version
    env.juliaup()
        .arg("link")
        .arg("r")
        .arg("+1.10.10")
        .assert()
        .success();

    // Update through the alias - should work and update the target
    env.juliaup()
        .arg("update")
        .arg("r")
        .assert()
        .success()
        .stderr(predicate::str::contains("Checking for new Julia versions"));
}

#[cfg(unix)]
fn read_config(env: &TestEnv) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(env.config_path()).unwrap()).unwrap()
}

/// https://github.com/JuliaLang/juliaup/issues/584: a linked channel can be
/// pointed somewhere else in one step, and stays the default.
#[cfg(unix)]
#[test]
fn link_force_replaces_linked_channel_and_keeps_default() {
    let env = TestEnv::new();

    env.juliaup()
        .args(["link", "dev", "/usr/bin/false"])
        .assert()
        .success();
    env.juliaup().args(["default", "dev"]).assert().success();

    env.juliaup()
        .args(["link", "--force", "dev", "/usr/bin/true"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Replacing"));

    let config = read_config(&env);
    assert_eq!(config["Default"], "dev");
    assert_eq!(
        config["InstalledChannels"]["dev"]["Command"],
        "/usr/bin/true"
    );
}

/// Without `--force` and without a terminal to ask, nothing is changed.
#[cfg(unix)]
#[test]
fn link_existing_channel_without_force_fails_non_interactively() {
    let env = TestEnv::new();

    env.juliaup()
        .args(["link", "dev", "/usr/bin/false"])
        .assert()
        .success();

    env.juliaup()
        .args(["link", "dev", "/usr/bin/true"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Channel name `dev` is already used")
                .and(predicate::str::contains("--force")),
        );

    let config = read_config(&env);
    assert_eq!(
        config["InstalledChannels"]["dev"]["Command"],
        "/usr/bin/false"
    );
}

#[cfg(unix)]
#[test]
fn link_force_switches_between_alias_and_linked_channel() {
    let env = TestEnv::new();

    env.juliaup()
        .args(["link", "stable", "+release"])
        .assert()
        .success();
    env.juliaup()
        .args(["link", "-f", "stable", "/usr/bin/true"])
        .assert()
        .success();
    assert_eq!(
        read_config(&env)["InstalledChannels"]["stable"]["Command"],
        "/usr/bin/true"
    );

    env.juliaup()
        .args(["link", "-f", "stable", "+release"])
        .assert()
        .success();
    assert_eq!(
        read_config(&env)["InstalledChannels"]["stable"]["Target"],
        "release"
    );
}

#[cfg(unix)]
#[test]
fn link_rejects_alias_to_itself() {
    let env = TestEnv::new();

    env.juliaup()
        .args(["link", "dev", "/usr/bin/false"])
        .assert()
        .success();
    env.juliaup()
        .args(["link", "--force", "dev", "+dev"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("alias of itself"));
}

/// With channel symlinks enabled, replacing a linked channel must replace its
/// `julia-<channel>` shim (see also #1171).
#[cfg(unix)]
#[test]
fn link_force_replaces_channel_shim() {
    let env = TestEnv::new();
    let bin_dir = assert_fs::TempDir::new().unwrap();
    let run = |args: &[&str]| {
        env.juliaup()
            .env("JULIAUP_BIN_DIR", bin_dir.path())
            .args(args)
            .assert()
            .success();
    };
    let shim = bin_dir.path().join("julia-dev");

    run(&["config", "channelsymlinks", "true"]);
    run(&["link", "dev", "/usr/bin/false"]);
    run(&["link", "--force", "dev", "/usr/bin/true"]);
    assert!(std::fs::read_to_string(&shim)
        .unwrap()
        .contains("/usr/bin/true"));

    // An alias has no shim, so the old one is removed.
    run(&["link", "--force", "dev", "+release"]);
    assert!(shim.symlink_metadata().is_err());
}

/// https://github.com/JuliaLang/juliaup/issues/1171: with channel symlinks
/// enabled, removing a linked channel failed with `Invalid argument (os error
/// 22)` because its `julia-<channel>` shim is a script, not a symlink.
#[cfg(unix)]
#[test]
fn remove_linked_channel_with_channel_symlinks() {
    let env = TestEnv::new();
    let bin_dir = assert_fs::TempDir::new().unwrap();
    let run = |args: &[&str]| {
        env.juliaup()
            .env("JULIAUP_BIN_DIR", bin_dir.path())
            .args(args)
            .assert()
            .success();
    };
    let shim = bin_dir.path().join("julia-dev");

    run(&["config", "channelsymlinks", "true"]);
    run(&["link", "dev", "/usr/bin/true"]);
    assert!(shim.is_file());

    run(&["remove", "dev"]);
    assert!(shim.symlink_metadata().is_err());
    assert!(read_config(&env)["InstalledChannels"].get("dev").is_none());
}
