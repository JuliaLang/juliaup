use predicates::prelude::predicate;

mod utils;
use utils::TestEnv;

#[test]
fn command_add() {
    let env = TestEnv::new();

    env.juliaup()
        .arg("add")
        .arg("1.6.4")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("add")
        .arg("nightly")
        .assert()
        .success()
        .stdout("");

    // Versioned nightly artifacts eventually expire. Exercise the newest
    // non-master nightly instead of pinning a retired release branch.
    let versioned_nightly_output = env
        .julia()
        .arg("+nightly")
        .arg("--startup-file=no")
        .arg("-e")
        .arg("print(VERSION.major, '.', VERSION.minor - 1, \"-nightly\")")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let versioned_nightly = std::str::from_utf8(&versioned_nightly_output).unwrap();

    env.juliaup()
        .arg("add")
        .arg(versioned_nightly)
        .assert()
        .success()
        .stdout("");

    env.julia()
        .arg("+1.6.4")
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.6.4");

    env.julia()
        .arg("+nightly")
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout(
            predicate::str::is_match(
                "^(0|[1-9]\\d*)\\.(0|[1-9]\\d*)\\.(0|[1-9]\\d*)-DEV\\.(0|[1-9]\\d*)",
            )
            .unwrap(),
        );
}

#[test]
fn command_add_pr_warning() {
    let env = TestEnv::new();

    // Test that adding a PR build shows a security warning with the PR URL
    // The command will fail because pr123 is old and won't have S3 artifacts, but we should see the warning
    env.juliaup()
        .arg("add")
        .arg("pr123")
        .write_stdin("n\n") // Decline codesigning prompt on macOS
        .assert()
        .failure() // Expect failure since the PR artifacts don't exist
        .stderr(predicate::str::contains(
            "WARNING: Note that unmerged PRs may not have been reviewed for security issues etc.",
        ))
        .stderr(predicate::str::contains(
            "Review code at https://github.com/JuliaLang/julia/pull/123",
        ));
}
