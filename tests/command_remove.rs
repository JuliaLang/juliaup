use predicates::boolean::PredicateBooleanExt;

mod utils;
use utils::{NightlyMetadataServer, TestEnv};

#[test]
fn command_remove() {
    let env = TestEnv::new();

    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").not());

    env.juliaup()
        .arg("add")
        .arg("1.6.4")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4"));

    env.juliaup()
        .arg("add")
        .arg("release")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").and(predicates::str::contains("release")));

    env.juliaup()
        .arg("remove")
        .arg("release")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").and(predicates::str::contains("release").not()));

    let metadata = NightlyMetadataServer::new();
    metadata
        .juliaup(&env)
        .arg("add")
        .arg("nightly")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").and(predicates::str::contains("-DEV")));

    env.juliaup()
        .arg("remove")
        .arg("nightly")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").and(predicates::str::contains("-DEV").not()));
}

fn link_channels(env: &TestEnv, names: &[&str]) {
    for name in names {
        env.juliaup()
            .arg("link")
            .arg(name)
            .arg("/usr/bin/false")
            .assert()
            .success();
    }
}

#[test]
fn command_remove_multiple_and_patterns() {
    let env = TestEnv::new();

    env.juliaup().arg("add").arg("1.10.10").assert().success();
    link_channels(
        &env,
        &[
            "pr62658", "pr63242", "pr63291", "pr1", "pr2", "dev", "1.10-dev",
        ],
    );

    // Several explicit channels at once
    env.juliaup()
        .arg("rm")
        .arg("pr62658")
        .arg("pr63242")
        .arg("pr63291")
        .assert()
        .success();

    env.juliaup().arg("status").assert().success().stdout(
        predicates::str::contains("pr62658")
            .or(predicates::str::contains("pr63242"))
            .or(predicates::str::contains("pr63291"))
            .not()
            .and(predicates::str::contains("pr1")),
    );

    // A pattern skips the default channel but removes the other matches
    env.juliaup()
        .arg("rm")
        .arg("1.10*")
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "'1.10.10' because it is currently configured as the default channel",
        ));

    env.juliaup().arg("status").assert().success().stdout(
        predicates::str::contains("1.10-dev")
            .not()
            .and(predicates::str::contains("1.10.10")),
    );

    // A pattern combined with an explicit channel
    env.juliaup()
        .arg("rm")
        .arg("pr*")
        .arg("dev")
        .assert()
        .success();

    env.juliaup().arg("status").assert().success().stdout(
        predicates::str::contains("pr1")
            .or(predicates::str::contains("pr2"))
            .or(predicates::str::contains("dev"))
            .not(),
    );
}

#[test]
fn command_remove_validates_before_removing() {
    let env = TestEnv::new();

    env.juliaup().arg("add").arg("1.10.10").assert().success();
    link_channels(&env, &["pr1"]);

    // An unknown channel fails the whole command and leaves the others installed
    env.juliaup()
        .arg("rm")
        .arg("pr1")
        .arg("notinstalled")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "'notinstalled' cannot be removed",
        ));

    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicates::str::contains("pr1"));

    // A pattern with no matches is an error
    env.juliaup()
        .arg("rm")
        .arg("nomatch*")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "No installed channels match 'nomatch*'",
        ));

    // An explicit default channel is still an error
    env.juliaup()
        .arg("rm")
        .arg("1.10.10")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "configured as the default channel",
        ));
}
