use predicates::prelude::*;
use serde_json::json;
use std::fs;

mod utils;
use utils::TestEnv;

#[test]
fn command_status() {
    let env = TestEnv::new();

    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(" Default  Channel  Version  Update \n-----------------------------------\n");

    env.juliaup()
        .arg("st")
        .assert()
        .success()
        .stdout(" Default  Channel  Version  Update \n-----------------------------------\n");
}

#[test]
fn command_status_formats_system_versions_for_display() {
    let env = TestEnv::new();
    let config_path = env.config_path();
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    fs::write(
        &config_path,
        serde_json::to_vec_pretty(&json!({
            "Default": "release",
            "InstalledVersions": {},
            "InstalledChannels": {
                "release": {
                    "Version": "1.0.0+0.x64.apple.darwin14"
                }
            },
            "Settings": {},
            "Overrides": []
        }))
        .unwrap(),
    )
    .unwrap();

    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("release"))
        .stdout(predicate::str::contains("1.0.0"))
        .stdout(predicate::str::contains("+0.").not());
}
