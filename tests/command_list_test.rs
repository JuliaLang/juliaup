use std::collections::HashMap;

use assert_cmd::Command;
use juliaup::utils::get_juliaserver_base_url;
use juliaup::versions_file::*;
use predicates::prelude::*;

#[test]
fn command_list() {
    let depot_dir = tempfile::Builder::new()
        .prefix("juliauptest")
        .tempdir()
        .unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("list")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::starts_with(" Channel").and(predicate::str::contains("release")));
}

#[test]
fn version_url() {
    let up_server_json_url = get_juliaserver_base_url()
        .unwrap()
        .join("./bin/versions.json");
    let json_versions: JsonVersion = ureq::get(up_server_json_url.unwrap().as_str())
        .call()
        .unwrap()
        .into_json()
        .unwrap();
    let mut exist_urls: HashMap<String, bool> = HashMap::new();
    for (_, v) in json_versions {
        for f in v.files {
            exist_urls.insert(f.url, true);
        }
    }

    let vdb = load_versions_db().unwrap();
    for (_, v) in vdb.available_versions {
        let full_url = format!(
            "https://julialang-s3.julialang.org/{}",
            v.url_path.to_string()
        );
        if !exist_urls.contains_key(&full_url) {
            println!("{}", &full_url);
        }
        // assert!(exist_urls.contains_key(&full_url), "{}", full_url);
    }
}
