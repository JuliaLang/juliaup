use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;
use std::path::Path;

#[test]
fn command_initial_setup() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    depot_dir
        .child(Path::new("juliaup"))
        .assert(predicate::path::missing());

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("46029ef5-0b73-4a71-bff3-d0d05de42aac")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(
            predicate::str::starts_with("    Checking for new Julia versions\n  Installing Julia 1.")
                .and(
                    predicate::str::contains("apple.darwin14")
                        .not()
                        .or(
                            predicate::str::contains("Checking standard library notarization")
                                .and(predicate::str::ends_with("done.\n")),
                        ),
                ),
        );

    depot_dir
        .child(Path::new("juliaup").join("juliaup.json"))
        .assert(predicate::path::exists());
}
