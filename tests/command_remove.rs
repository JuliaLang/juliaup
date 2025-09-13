use predicates::boolean::PredicateBooleanExt;

mod utils;
use utils::TestEnv;

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

    env.juliaup()
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
