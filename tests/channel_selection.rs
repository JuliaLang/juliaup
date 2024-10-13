use assert_cmd::Command;

use anyhow::Result;
use juliaup::config_file::{load_config_db, JuliaupConfigChannel};
use juliaup::global_paths::get_paths_from_home_path;
use normpath::PathExt;
use std::path::PathBuf;

// Simpler reimplementation of get_julia_path_from_channel from julialauncher.rs to help link channels
fn get_julia_path_from_channel(
    requested_channel: &str,
    juliaup_depot_path: PathBuf,
) -> Result<PathBuf> {
    let paths = get_paths_from_home_path(juliaup_depot_path)?;
    let config_file = load_config_db(&paths)?;
    let config_data = config_file.data;

    let juliaupconfig_path = paths.juliaupconfig.as_path();

    let channel_info = config_data
        .installed_channels
        .get(requested_channel)
        .unwrap();

    let path: &String = if let JuliaupConfigChannel::SystemChannel { version } = channel_info {
        &config_data.installed_versions.get(version).unwrap().path
    } else {
        panic!("whoops")
    };

    let absolute_path = juliaupconfig_path
        .parent()
        .unwrap() // unwrap OK because there should always be a parent
        .join(path)
        .join("bin")
        .join(format!("julia{}", std::env::consts::EXE_SUFFIX))
        .normalize()?;

    return Ok(absolute_path.into_path_buf());
}

#[test]
fn channel_selection() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.7.3")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("default")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("1.6.7");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8.5")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("1.8.5");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.3")
        .assert()
        .success()
        .stdout("1.7.3");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8.5")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.3")
        .assert()
        .success()
        .stdout("1.8.5");

    // Now testing incorrect channels

    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8.6")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure()
        .stderr("ERROR: Invalid Juliaup channel `1.8.6`. Please run `juliaup list` to get a list of valid channels and versions.\n");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr(
            "ERROR: Invalid Juliaup channel `1.7.4` from environment variable JULIAUP_CHANNEL. Please run `juliaup list` to get a list of valid channels and versions.\n",
        );

    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8.6")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr("ERROR: Invalid Juliaup channel `1.8.6`. Please run `juliaup list` to get a list of valid channels and versions.\n");

    // https://github.com/JuliaLang/juliaup/issues/766
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8.2")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr("ERROR: `1.8.2` is not installed. Please run `juliaup add 1.8.2` to install channel or version.\n");

    // https://github.com/JuliaLang/juliaup/issues/820
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+nightly")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr("ERROR: `nightly` is not installed. Please run `juliaup add nightly` to install channel or version.\n");

    // Now testing short channel matching
    // At this point, installed channels are: 1.6.7, 1.7.3, 1.8.5

    // Test that incomplete number matching does not autocomplete:
    // https://github.com/JuliaLang/juliaup/pull/838#issuecomment-2206640506
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8")
        .arg("-v")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure();

    // Test that completion works only when it should for words
    let linked_julia_path =
        get_julia_path_from_channel("1.6.7", depot_dir.path().to_path_buf().join("juliaup"))
            .unwrap();
    let linked_julia_version = linked_julia_path.to_str().unwrap();
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("ra")
        .arg(linked_julia_version)
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("julia")
        .unwrap()
        .arg("+r")
        .arg("-v")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("rb")
        .arg(linked_julia_version)
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("julia")
        .unwrap()
        .arg("+r")
        .arg("-v")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure();
}
