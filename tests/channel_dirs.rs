mod utils;
use utils::TestEnv;

#[test]
fn channel_dirs_follow_installed_channels() {
    let env = TestEnv::new();
    env.juliaup().args(["add", "1.10.10"]).assert().success();

    let channels = env.depot_path().join("juliaup").join("channels");
    std::fs::create_dir_all(channels.join("stray_dir")).unwrap();
    std::fs::write(channels.join("stray_file"), "").unwrap();

    env.juliaup()
        .args(["link", "stable", "+1.10.10"])
        .assert()
        .success();

    assert!(channels.join("stray_dir").symlink_metadata().is_err());
    assert!(channels.join("stray_file").symlink_metadata().is_err());
    let julia = std::path::Path::new("bin").join(format!("julia{}", std::env::consts::EXE_SUFFIX));
    assert!(channels.join("1.10.10").join(&julia).is_file());
    assert!(channels.join("stable").join(&julia).is_file());

    env.juliaup().args(["remove", "stable"]).assert().success();
    assert!(channels.join("stable").symlink_metadata().is_err());
    assert!(channels.join("1.10.10").join(&julia).is_file());
}
