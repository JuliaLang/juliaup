use predicates::prelude::*;

mod utils;
use utils::TestEnv;

#[test]
fn command_list() {
    let env = TestEnv::new();

    env.juliaup()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::starts_with(" Channel").and(predicate::str::contains("release")))
        .stdout(predicate::str::contains("nightly"))
        .stdout(predicate::str::contains("x.y-nightly"))
        .stdout(predicate::str::contains("pr{number}"));

    env.juliaup()
        .arg("ls")
        .assert()
        .success()
        .stdout(predicate::str::starts_with(" Channel").and(predicate::str::contains("release")));
}
