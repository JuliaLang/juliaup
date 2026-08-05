use predicates::prelude::*;
use serde_json::json;
use std::fs;

mod utils;
use utils::TestEnv;

#[test]
fn command_gc() {
    let env = TestEnv::new();

    env.juliaup().arg("add").arg("1.6.7").assert().success();

    env.juliaup()
        .arg("link")
        .arg("julib")
        .arg("julib")
        .assert()
        .success();

    env.juliaup()
        .arg("link")
        .arg("julic")
        .arg("julic")
        .assert()
        .success();

    env.juliaup().arg("status").assert().success().stdout(
        predicate::str::contains("\n")
            .count(5)
            .and(predicate::str::contains("julic"))
            .and(predicate::str::contains("julib")),
    );

    env.juliaup().arg("gc").assert().success();

    env.juliaup().arg("status").assert().success().stdout(
        predicate::str::contains("\n")
            .count(5)
            .and(predicate::str::contains("julic"))
            .and(predicate::str::contains("julib")),
    );

    env.juliaup()
        .arg("gc")
        .arg("--prune-linked")
        .assert()
        .success();

    env.juliaup().arg("status").assert().success().stdout(
        predicate::str::contains("\n")
            .count(3)
            .and(predicate::str::contains("julic").not())
            .and(predicate::str::contains("julib").not()),
    );
}

#[test]
fn command_gc_formats_removed_versions_for_display() {
    let env = TestEnv::new();
    let config_path = env.config_path();
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    fs::write(
        &config_path,
        serde_json::to_vec_pretty(&json!({
            "Default": null,
            "InstalledVersions": {
                "1.12.6+0.aarch64.apple.darwin14": {
                    "Path": "missing-version-directory"
                }
            },
            "InstalledChannels": {},
            "Settings": {},
            "Overrides": []
        }))
        .unwrap(),
    )
    .unwrap();

    env.juliaup()
        .arg("gc")
        .assert()
        .success()
        .stderr(predicate::str::contains("Removed Julia 1.12.6"))
        .stderr(predicate::str::contains("+0.").not());
}
