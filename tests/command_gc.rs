use predicates::prelude::*;

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
