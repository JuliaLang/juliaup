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

#[test]
fn command_gc_keeps_versions_used_by_apps() {
    let env = TestEnv::new();
    let juliauphome = env.depot_path().join("juliaup");

    env.juliaup().arg("add").arg("1.6.7").assert().success();

    let version_dir = std::fs::read_dir(&juliauphome)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("julia-1.6.7")
        })
        .unwrap();
    // (the macOS versions are inside an app bundle)
    let julia = find_julia(&version_dir).unwrap();

    // A Julia app installed with that julia
    let apps = env.depot_path().join("environments").join("apps");
    std::fs::create_dir_all(&apps).unwrap();
    let app_manifest = apps.join("AppManifest.toml");
    std::fs::write(
        &app_manifest,
        format!(
            "manifest_format = \"2.0\"\n\n[[deps.MyApp]]\nuuid = \"00000000-0000-0000-0000-000000000001\"\n\n    [deps.MyApp.apps.myapp]\n    julia_command = {}\n",
            toml_string(&julia)
        ),
    )
    .unwrap();

    // Removing the channel keeps the version
    env.juliaup()
        .arg("link")
        .arg("julib")
        .arg("julib")
        .assert()
        .success();
    env.juliaup().arg("default").arg("julib").assert().success();
    env.juliaup()
        .arg("remove")
        .arg("1.6.7")
        .assert()
        .success()
        .stderr(predicate::str::contains("Keeping"));
    assert!(julia.exists());

    // Once no app uses it anymore the version is removed
    std::fs::remove_file(&app_manifest).unwrap();
    env.juliaup().arg("gc").assert().success();
    assert!(!version_dir.exists());
}

fn find_julia(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let exe = if cfg!(windows) { "julia.exe" } else { "julia" };
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(julia) = find_julia(&path) {
                return Some(julia);
            }
        } else if path.file_name().unwrap() == exe
            && path.parent().unwrap().file_name().unwrap() == "bin"
        {
            return Some(path);
        }
    }
    None
}

fn toml_string(path: &std::path::Path) -> String {
    format!("'{}'", path.display())
}
