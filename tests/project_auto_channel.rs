use std::fs;
use std::path::PathBuf;

mod utils;
use utils::TestEnv;

fn write_project(project_dir: &PathBuf, manifest_version: &str, compat: Option<&str>) {
    fs::create_dir_all(project_dir).unwrap();

    fs::write(
        project_dir.join("Project.toml"),
        format!(
            r#"
name = "AutoProject"
uuid = "00000000-0000-0000-0000-000000000001"
version = "0.1.0"

{}
"#,
            compat
                .map(|c| format!("[compat]\njulia = \"{}\"", c))
                .unwrap_or_default()
        ),
    )
    .unwrap();

    fs::write(
        project_dir.join("Manifest.toml"),
        format!(
            r#"
julia_version = "{}"
"#,
            manifest_version
        ),
    )
    .unwrap();
}

fn install_channel(env: &TestEnv, channel: &str) {
    env.juliaup().arg("add").arg(channel).assert().success();
}

#[test]
#[ignore]
fn end_to_end_manifest_selection() {
    let env = TestEnv::new();
    install_channel(&env, "1.8.2");

    let project_dir = env.depot_path().join("manifest_project");
    write_project(&project_dir, "1.8.2", None);

    env.julia()
        .arg("+auto")
        .arg(format!(
            "--project={}",
            project_dir.as_os_str().to_string_lossy()
        ))
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.8.2");
}
