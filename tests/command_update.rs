use predicates::boolean::PredicateBooleanExt;
use predicates::str::contains;
use serde_json::Value;
use std::fs;

mod utils;
use utils::TestEnv;

#[test]
fn command_update() {
    let env = TestEnv::new();

    env.juliaup().arg("update").assert().success().stdout("");

    env.juliaup().arg("up").assert().success().stdout("");
}

#[test]
fn command_update_alias_works() {
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

    // Update the alias - should succeed and update the target
    env.juliaup().arg("update").arg("r").assert().success();
}

#[test]
fn command_update_all_with_alias() {
    let env = TestEnv::new();

    // First install a Julia version to create an alias to
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Create an alias to the installed version - this reproduces the original bug scenario
    env.juliaup()
        .arg("link")
        .arg("r")
        .arg("+1.10.10")
        .assert()
        .success();

    env.juliaup().arg("update").assert().success();
}

#[test]
fn command_update_outdated_channel() {
    let env = TestEnv::new();

    env.juliaup().arg("add").arg("1.10").assert().success();
    env.juliaup().arg("add").arg("1.10.9").assert().success();

    let config_path = env.config_path();
    let config_content = fs::read_to_string(&config_path).expect("Failed to read config file");
    let mut config: Value = serde_json::from_str(&config_content).expect("Failed to parse config");

    // Find the actual version key for 1.10.9 in InstalledVersions
    let installed_versions = config["InstalledVersions"]
        .as_object()
        .expect("InstalledVersions should be an object");

    let version_1_10_9_key = installed_versions
        .keys()
        .find(|k| k.starts_with("1.10.9"))
        .expect("Should have 1.10.9 version installed")
        .clone();

    let channels = config["InstalledChannels"]
        .as_object_mut()
        .expect("InstalledChannels should be an object");

    if let Some(channel_110) = channels.get_mut("1.10") {
        if let Some(version) = channel_110.get_mut("Version") {
            *version = Value::String(version_1_10_9_key);
        }
    }

    fs::write(
        &config_path,
        serde_json::to_string_pretty(&config).expect("Failed to serialize config"),
    )
    .expect("Failed to write config file");

    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(contains("1.10").and(contains("Update")));

    // Update hints should not appear when using julia noninteractively.
    // (ideally we'd test the interactive case too, but that's tricky in CI)
    env.julia()
        .arg("+1.10")
        .arg("--version")
        .assert()
        .success()
        .stderr(
            contains("latest version")
                .not()
                .and(contains("juliaup update").not())
                .and(contains("You currently have").not()),
        );

    env.julia()
        .arg("+1.10")
        .arg("-e")
        .arg("1+1")
        .assert()
        .success()
        .stderr(
            contains("latest version")
                .not()
                .and(contains("juliaup update").not())
                .and(contains("You currently have").not()),
        );

    env.juliaup().arg("update").arg("1.10").assert().success();

    let updated_config_content =
        fs::read_to_string(&config_path).expect("Failed to read config file after update");
    let updated_config: Value =
        serde_json::from_str(&updated_config_content).expect("Failed to parse updated config");

    let updated_channels = updated_config["InstalledChannels"]
        .as_object()
        .expect("InstalledChannels should be an object");

    let new_version = updated_channels
        .get("1.10")
        .and_then(|c| c.get("Version"))
        .and_then(|v| v.as_str())
        .expect("1.10 channel should have a version");

    assert!(
        new_version.starts_with("1.10.10"),
        "Channel 1.10 should have been updated to 1.10.10 (was {})",
        new_version
    );
}

#[test]
fn command_update_all_outdated_channels() {
    let env = TestEnv::new();

    env.juliaup().arg("add").arg("1.10").assert().success();
    env.juliaup().arg("add").arg("1.10.9").assert().success();
    env.juliaup().arg("add").arg("1.11").assert().success();
    env.juliaup().arg("add").arg("1.11.1").assert().success();

    let config_path = env.config_path();
    let config_content = fs::read_to_string(&config_path).expect("Failed to read config file");
    let mut config: Value = serde_json::from_str(&config_content).expect("Failed to parse config");

    let channels = config["InstalledChannels"]
        .as_object_mut()
        .expect("InstalledChannels should be an object");

    if let Some(channel_110) = channels.get_mut("1.10") {
        if let Some(version) = channel_110.get_mut("Version") {
            *version = Value::String("1.10.9".to_string());
        }
    }

    if let Some(channel_111) = channels.get_mut("1.11") {
        if let Some(version) = channel_111.get_mut("Version") {
            *version = Value::String("1.11.1".to_string());
        }
    }

    fs::write(
        &config_path,
        serde_json::to_string_pretty(&config).expect("Failed to serialize config"),
    )
    .expect("Failed to write config file");

    env.juliaup().arg("update").assert().success();

    let updated_config_content =
        fs::read_to_string(&config_path).expect("Failed to read config file after update");
    let updated_config: Value =
        serde_json::from_str(&updated_config_content).expect("Failed to parse updated config");

    let updated_channels = updated_config["InstalledChannels"]
        .as_object()
        .expect("InstalledChannels should be an object");

    let new_version_110 = updated_channels
        .get("1.10")
        .and_then(|c| c.get("Version"))
        .and_then(|v| v.as_str())
        .expect("1.10 channel should have a version");

    let new_version_111 = updated_channels
        .get("1.11")
        .and_then(|c| c.get("Version"))
        .and_then(|v| v.as_str())
        .expect("1.11 channel should have a version");

    assert!(
        new_version_110.starts_with("1.10.10"),
        "Channel 1.10 should have been updated to 1.10.10 (was {})",
        new_version_110
    );

    assert!(
        new_version_111.starts_with("1.11."),
        "Channel 1.11 should have been updated to latest 1.11.x (was {})",
        new_version_111
    );
    assert!(
        !new_version_111.starts_with("1.11.1"),
        "Channel 1.11 should have been updated past 1.11.1 (was {})",
        new_version_111
    );
}
