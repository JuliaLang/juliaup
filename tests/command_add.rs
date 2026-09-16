use predicates::boolean::PredicateBooleanExt;
use predicates::prelude::predicate;

mod utils;
use utils::{NightlyMetadataServer, TestEnv};

#[test]
fn command_add() {
    let env = TestEnv::new();

    env.juliaup()
        .arg("add")
        .arg("1.6.4")
        .assert()
        .success()
        .stdout("");

    let metadata = NightlyMetadataServer::new();
    metadata
        .juliaup(&env)
        .arg("add")
        .arg("nightly")
        .assert()
        .success()
        .stdout("");

    metadata
        .juliaup(&env)
        .args(["add", "1.0-nightly"])
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
fn command_add_unknown_nightly_variant() {
    let env = TestEnv::new();

    let metadata = NightlyMetadataServer::new();
    metadata
        .juliaup(&env)
        .arg("add")
        .arg("nightly+bogus")
        .assert()
        .failure()
        .stderr(predicate::str::contains("'nightly' has no '+bogus' build"));

    // Variants are only produced for nightlies.
    env.juliaup()
        .arg("add")
        .arg("release+opt")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "build variants such as `+opt` are only available for nightly channels",
        ));
}

// The `nogpl` variant is built for linux/x86_64 (and macOS and Windows on
// x86_64, where this test would additionally exercise code signing).
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn command_add_nightly_variant() {
    let env = TestEnv::new();

    let metadata = NightlyMetadataServer::new();
    metadata
        .juliaup(&env)
        .arg("add")
        .arg("nightly+nogpl")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Installing Julia latest-linuxnogpl-x86_64",
        ));

    env.julia()
        .arg("+nightly+nogpl")
        .arg("--startup-file=no")
        .arg("-e")
        .arg("print(Base.USE_GPL_LIBS)")
        .assert()
        .success()
        .stdout("false");

    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("nightly+nogpl"));
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

#[test]
fn command_add_reuses_installed_version() {
    let env = TestEnv::new();

    env.juliaup()
        .arg("add")
        .arg("1.10")
        .assert()
        .success()
        .stderr(predicate::str::contains("Installing Julia 1.10."));

    let version_output = env
        .julia()
        .arg("+1.10")
        .arg("--startup-file=no")
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let version = std::str::from_utf8(&version_output).unwrap();

    // The version is already installed for the `1.10` channel, so adding it
    // by exact version must only add the channel and not download it again.
    env.juliaup()
        .arg("add")
        .arg(version)
        .assert()
        .success()
        .stderr(predicate::str::contains("Installing").not());

    env.julia()
        .arg(format!("+{}", version))
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout(version.to_string());
}
