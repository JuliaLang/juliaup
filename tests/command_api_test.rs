mod utils;
use utils::TestEnv;

#[test]
fn api_getconfig1_basic() {
    let env = TestEnv::new();

    // Install a Julia version
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Get the config via API
    let output = env
        .juliaup()
        .arg("api")
        .arg("getconfig1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_str = std::str::from_utf8(&output).unwrap();
    let json: serde_json::Value = serde_json::from_str(json_str).unwrap();

    // Check that we get valid JSON with expected structure
    assert!(json.get("DefaultChannel").is_some());
    assert!(json.get("OtherChannels").is_some());
}

#[test]
fn api_getconfig1_alias_returns_valid_file_path() {
    let env = TestEnv::new();

    // Install a Julia version
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Create an alias to the installed version
    env.juliaup()
        .arg("link")
        .arg("myalias")
        .arg("+1.10.10")
        .assert()
        .success();

    // Get the config via API
    let output = env
        .juliaup()
        .arg("api")
        .arg("getconfig1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_str = std::str::from_utf8(&output).unwrap();
    let json: serde_json::Value = serde_json::from_str(json_str).unwrap();

    // Find the alias channel in the output
    let channels = json["OtherChannels"].as_array().unwrap();
    let alias_channel = channels
        .iter()
        .find(|c| c["Name"].as_str() == Some("myalias"))
        .expect("Could not find alias channel in API output");

    // The File field should be an actual file path (containing "julia" executable),
    // not "alias-to-1.10.10"
    let file_path = alias_channel["File"].as_str().unwrap();
    assert!(
        !file_path.starts_with("alias-to-"),
        "File path should not be 'alias-to-...' but was: {}",
        file_path
    );
    assert!(
        file_path.contains("julia"),
        "File path should contain 'julia' but was: {}",
        file_path
    );

    // Version should be the actual version, not "alias to ..."
    let version = alias_channel["Version"].as_str().unwrap();
    assert!(
        !version.starts_with("alias to"),
        "Version should not be 'alias to ...' but was: {}",
        version
    );
    assert_eq!(version, "1.10.10");
}

#[test]
fn api_getconfig1_alias_with_args() {
    let env = TestEnv::new();

    // Install a Julia version
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Create an alias with additional arguments
    env.juliaup()
        .arg("link")
        .arg("myalias")
        .arg("+1.10.10")
        .arg("--")
        .arg("--project=@.")
        .assert()
        .success();

    // Get the config via API
    let output = env
        .juliaup()
        .arg("api")
        .arg("getconfig1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_str = std::str::from_utf8(&output).unwrap();
    let json: serde_json::Value = serde_json::from_str(json_str).unwrap();

    // Find the alias channel in the output
    let channels = json["OtherChannels"].as_array().unwrap();
    let alias_channel = channels
        .iter()
        .find(|c| c["Name"].as_str() == Some("myalias"))
        .expect("Could not find alias channel in API output");

    // The File field should be an actual file path
    let file_path = alias_channel["File"].as_str().unwrap();
    assert!(!file_path.starts_with("alias-to-"));

    // Args should include the alias arguments
    let args = alias_channel["Args"].as_array().unwrap();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].as_str().unwrap(), "--project=@.");
}

#[test]
fn api_getconfig1_alias_to_system_channel() {
    let env = TestEnv::new();

    // Install release channel first to ensure it exists
    env.juliaup().arg("add").arg("release").assert().success();

    // Create an alias to a system channel (release)
    env.juliaup()
        .arg("link")
        .arg("myrelease")
        .arg("+release")
        .assert()
        .success();

    // Get the config via API
    let output = env
        .juliaup()
        .arg("api")
        .arg("getconfig1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_str = std::str::from_utf8(&output).unwrap();
    let json: serde_json::Value = serde_json::from_str(json_str).unwrap();

    // Find the alias channel in the output
    let channels = json["OtherChannels"].as_array().unwrap();
    let alias_channel = channels
        .iter()
        .find(|c| c["Name"].as_str() == Some("myrelease"))
        .expect("Could not find alias channel in API output");

    // The File field should be an actual file path, not "alias-to-release"
    let file_path = alias_channel["File"].as_str().unwrap();
    assert!(
        !file_path.starts_with("alias-to-"),
        "File path should not be 'alias-to-...' but was: {}",
        file_path
    );
    assert!(file_path.contains("julia"));
}
