use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

#[test]
fn command_override_status_test() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(" Path  Channel \n---------------\n");
}

#[test]
fn command_override_cur_dir_test() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    let or_dir = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("default")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("1.6.7");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("set")
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .current_dir(&or_dir)
        .assert()
        .success();

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("1.8.5");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("unset")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .current_dir(&or_dir)
        .assert()
        .success();

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("1.6.7");
}

#[test]
fn command_override_arg_test() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    let or_dir = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("default")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("1.6.7");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("set")
        .arg("--path")
        .arg(or_dir.as_os_str())
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("1.8.5");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("unset")
        .arg("--path")
        .arg(or_dir.as_os_str())
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .current_dir(&or_dir)
        .assert()
        .success()
        .stdout("1.6.7");
}

#[test]
fn command_override_overlap_test() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    let or_dir_parent = assert_fs::TempDir::new().unwrap();
    let or_dir_child = or_dir_parent.join("child");
    std::fs::create_dir_all(&or_dir_child).unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.7.3")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("default")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("set")
        .arg("--path")
        .arg(or_dir_parent.as_os_str())
        .arg("1.7.3")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("set")
        .arg("--path")
        .arg(&or_dir_child.as_os_str())
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .current_dir(&or_dir_parent)
        .assert()
        .success()
        .stdout("1.7.3");

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .current_dir(&or_dir_child)
        .assert()
        .success()
        .stdout("1.8.5");
}

#[test]
fn command_override_delete_empty_test() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    let or_dir1 = assert_fs::TempDir::new().unwrap();
    let or_dir2 = assert_fs::TempDir::new().unwrap();
    let or_dir3 = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("set")
        .arg("--path")
        .arg(or_dir1.as_os_str())
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("set")
        .arg("--path")
        .arg(or_dir2.as_os_str())
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("set")
        .arg("--path")
        .arg(or_dir3.as_os_str())
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("unset")
        .arg("--nonexistent")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicates::ord::eq(" Path  Channel \n---------------\n").not());

    std::fs::remove_dir(or_dir1).unwrap();
    std::fs::remove_dir(or_dir2).unwrap();
    std::fs::remove_dir(or_dir3).unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("unset")
        .arg("--nonexistent")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("override")
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(" Path  Channel \n---------------\n");
}
