#![cfg(all(feature = "binjuliainstaller", feature = "selfupdate"))]

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

#[test]
fn installer_help_documents_noninteractive_options() {
    cargo_bin_cmd!("juliainstaller")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "--add-to-path <yes|no|0|1>",
        ))
        .stdout(predicate::str::contains(
            "Add the Juliaup bin directory to PATH startup files [default: yes]",
        ))
        .stdout(predicate::str::contains(
            "--background-selfupdate <MINUTES>",
        ))
        .stdout(predicate::str::contains(
            "Check for Juliaup self-updates in the background every MINUTES minutes [default: 0]",
        ))
        .stdout(predicate::str::contains(
            "--startup-selfupdate <MINUTES>",
        ))
        .stdout(predicate::str::contains(
            "Check for Juliaup self-updates when Julia starts every MINUTES minutes [default: 1440]",
        ));
}
