use predicates::prelude::PredicateBooleanExt;
use predicates::str::starts_with;

mod utils;
use utils::TestEnv;

#[test]
fn command_override_status_test() {
    let env = TestEnv::new();

    env.juliaup()
        .arg("add")
        .arg("1.6.7")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("add")
        .arg("1.8.5")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("override")
        .arg("status")
        .assert()
        .success()
        .stdout(" Path  Channel \n---------------\n");
}

#[test]
fn command_override_cur_dir_test() {
    let env = TestEnv::new();

    let or_dir = assert_fs::TempDir::new().unwrap();

    env.juliaup()
        .arg("add")
        .arg("1.6.7")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("add")
        .arg("1.8.5")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("default")
        .arg("1.6.7")
        .assert()
        .success()
        .stdout("");

    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("1.6.7");

    env.juliaup()
        .arg("override")
        .arg("set")
        .arg("1.6.7")
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("")
        .stderr(starts_with("Override set to '1.6.7'"));

    env.juliaup()
        .arg("override")
        .arg("set")
        .arg("1.6.7")
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("")
        .stderr(starts_with("Override already set to '1.6.7'"));

    env.juliaup()
        .arg("override")
        .arg("set")
        .arg("1.8.5")
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("")
        .stderr(starts_with("Override changed from '1.6.7' to '1.8.5'"));

    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("1.8.5");

    env.juliaup()
        .arg("override")
        .arg("unset")
        .current_dir(&or_dir)
        .assert()
        .success();

    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("1.6.7");
}

#[test]
fn command_override_arg_test() {
    let env = TestEnv::new();

    let or_dir = assert_fs::TempDir::new().unwrap();

    env.juliaup()
        .arg("add")
        .arg("1.6.7")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("add")
        .arg("1.8.5")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("default")
        .arg("1.6.7")
        .assert()
        .success()
        .stdout("");

    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("1.6.7");

    env.juliaup()
        .arg("override")
        .arg("set")
        .arg("--path")
        .arg(or_dir.as_os_str())
        .arg("1.8.5")
        .assert()
        .success();

    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("1.8.5");

    env.juliaup()
        .arg("override")
        .arg("unset")
        .arg("--path")
        .arg(or_dir.as_os_str())
        .assert()
        .success();

    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("1.6.7");
}

#[test]
fn command_override_overlap_test() {
    let env = TestEnv::new();

    let or_dir_parent = assert_fs::TempDir::new().unwrap();
    let or_dir_child = or_dir_parent.join("child");
    std::fs::create_dir_all(&or_dir_child).unwrap();

    env.juliaup()
        .arg("add")
        .arg("1.6.7")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("add")
        .arg("1.7.3")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("add")
        .arg("1.8.5")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("default")
        .arg("1.6.7")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("override")
        .arg("set")
        .arg("--path")
        .arg(or_dir_parent.as_os_str())
        .arg("1.7.3")
        .assert()
        .success();

    env.juliaup()
        .arg("override")
        .arg("set")
        .arg("--path")
        .arg(or_dir_child.as_os_str())
        .arg("1.8.5")
        .assert()
        .success();

    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .current_dir(&or_dir_parent)
        .assert()
        .success()
        .stdout("1.7.3");

    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .current_dir(&or_dir_child)
        .assert()
        .success()
        .stdout("1.8.5");
}

#[test]
fn command_override_delete_empty_test() {
    let env = TestEnv::new();

    let or_dir1 = assert_fs::TempDir::new().unwrap();
    let or_dir2 = assert_fs::TempDir::new().unwrap();
    let or_dir3 = assert_fs::TempDir::new().unwrap();

    env.juliaup()
        .arg("add")
        .arg("1.6.7")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("override")
        .arg("set")
        .arg("--path")
        .arg(or_dir1.as_os_str())
        .arg("1.6.7")
        .assert()
        .success();

    env.juliaup()
        .arg("override")
        .arg("set")
        .arg("--path")
        .arg(or_dir2.as_os_str())
        .arg("1.6.7")
        .assert()
        .success();

    env.juliaup()
        .arg("override")
        .arg("set")
        .arg("--path")
        .arg(or_dir3.as_os_str())
        .arg("1.6.7")
        .assert()
        .success();

    env.juliaup()
        .arg("override")
        .arg("unset")
        .arg("--nonexistent")
        .assert()
        .success();

    env.juliaup()
        .arg("override")
        .arg("status")
        .assert()
        .success()
        .stdout(predicates::ord::eq(" Path  Channel \n---------------\n").not());

    std::fs::remove_dir(or_dir1).unwrap();
    std::fs::remove_dir(or_dir2).unwrap();
    std::fs::remove_dir(or_dir3).unwrap();

    env.juliaup()
        .arg("override")
        .arg("unset")
        .arg("--nonexistent")
        .assert()
        .success();

    env.juliaup()
        .arg("override")
        .arg("status")
        .assert()
        .success()
        .stdout(" Path  Channel \n---------------\n");
}
