#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use flate2::{write::GzEncoder, Compression};
use predicates::prelude::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::Duration;
use tiny_http::{Header, Response, Server};

mod utils;
use utils::TestEnv;

const CATALOG: &str = r#"{
    "nightly": {"files": [], "variants": [{
        "os": "linux", "arch": "x86_64", "triplet": "x86_64-linux-gnu",
        "kind": "archive", "extension": "tar.gz", "variants": ["opt"],
        "url": "https://julialangnightlies-s3.julialang.org/bin/opt.tar.gz"
    }]}
}"#;

struct Mirror {
    url: String,
    revision: Arc<AtomicUsize>,
    catalog_available: Arc<AtomicBool>,
    etags: Arc<AtomicBool>,
    catalog: Arc<Mutex<String>>,
    payloads: Arc<AtomicUsize>,
    artifacts: Arc<AtomicUsize>,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Mirror {
    fn new() -> Self {
        let server = Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr());
        let revision = Arc::new(AtomicUsize::new(1));
        let catalog_available = Arc::new(AtomicBool::new(true));
        let etags = Arc::new(AtomicBool::new(true));
        let catalog = Arc::new(Mutex::new(CATALOG.to_string()));
        let payloads = Arc::new(AtomicUsize::new(0));
        let artifacts = Arc::new(AtomicUsize::new(0));
        let stopped = Arc::new(AtomicBool::new(false));
        let worker = {
            let revision = revision.clone();
            let catalog_available = catalog_available.clone();
            let etags = etags.clone();
            let catalog = catalog.clone();
            let payloads = payloads.clone();
            let artifacts = artifacts.clone();
            let stopped = stopped.clone();
            std::thread::spawn(move || {
                while !stopped.load(Ordering::SeqCst) {
                    let Some(request) = server.recv_timeout(Duration::from_millis(50)).unwrap()
                    else {
                        continue;
                    };
                    if request.url() == "/bin/opt.tar.gz" {
                        artifacts.fetch_add(1, Ordering::SeqCst);
                    }
                    let rev = revision.load(Ordering::SeqCst);
                    let catalog = catalog.lock().unwrap().clone();
                    let (body, status) = match request.url() {
                        "/bin/nightlies.json" => {
                            payloads.fetch_add(1, Ordering::SeqCst);
                            if catalog_available.load(Ordering::SeqCst) {
                                (catalog.as_bytes().to_vec(), 200)
                            } else {
                                (Vec::new(), 404)
                            }
                        }
                        "/juliaup/RELEASECHANNELDBVERSION" => (b"0.0.0".to_vec(), 200),
                        "/bin/opt.tar.gz" => (
                            if etags.load(Ordering::SeqCst) {
                                archive(rev)
                            } else {
                                b"not an archive".to_vec()
                            },
                            200,
                        ),
                        _ => (Vec::new(), 404),
                    };
                    let mut response = Response::from_data(body).with_status_code(status);
                    if request.url() == "/bin/opt.tar.gz" && etags.load(Ordering::SeqCst) {
                        response
                            .add_header(Header::from_bytes("ETag", format!("\"{rev}\"")).unwrap());
                    }
                    // A downloader may reject headers before consuming the body.
                    let _ = request.respond(response);
                }
            })
        };
        Self {
            url,
            revision,
            catalog_available,
            etags,
            catalog,
            payloads,
            artifacts,
            stopped,
            worker: Some(worker),
        }
    }

    fn command(&self, env: &TestEnv) -> assert_cmd::Command {
        let mut cmd = env.juliaup();
        cmd.env("JULIAUP_SERVER", &self.url)
            .env("JULIAUP_NIGHTLY_SERVER", &self.url);
        cmd
    }
}

impl Drop for Mirror {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn archive(revision: usize) -> Vec<u8> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let script = format!("#!/bin/sh\nprintf '1.14.0-DEV.{revision}'\n");
    let mut header = tar::Header::new_gnu();
    header.set_size(script.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    builder
        .append_data(&mut header, "julia/bin/julia", script.as_bytes())
        .unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

#[test]
fn catalog_variant_install_and_update_through_mirror() {
    let env = TestEnv::new();
    let mirror = Mirror::new();
    mirror
        .command(&env)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("nightly+opt~x64"))
        .stdout(predicate::str::contains("nightly+assert").not());
    mirror
        .command(&env)
        .args(["add", "nightly+opt"])
        .assert()
        .success();
    env.julia()
        .args(["+nightly+opt", "--version"])
        .assert()
        .success()
        .stdout("1.14.0-DEV.1");
    let requests = mirror.payloads.load(Ordering::SeqCst);
    mirror.revision.store(2, Ordering::SeqCst);
    mirror
        .command(&env)
        .args(["update", "nightly+opt"])
        .assert()
        .success();
    env.julia()
        .args(["+nightly+opt", "--version"])
        .assert()
        .success()
        .stdout("1.14.0-DEV.2");
    assert_eq!(mirror.payloads.load(Ordering::SeqCst), requests);
    age_cache(&env);
    mirror.command(&env).arg("update").assert().success();
    assert_eq!(mirror.payloads.load(Ordering::SeqCst), requests + 1);
    // Failed discovery must not stop an installed artifact updating.
    age_cache(&env);
    mirror.catalog_available.store(false, Ordering::SeqCst);
    mirror.revision.store(3, Ordering::SeqCst);
    mirror.command(&env).arg("update").assert().success();
    env.julia()
        .args(["+nightly+opt", "--version"])
        .assert()
        .success()
        .stdout("1.14.0-DEV.3");
    mirror
        .command(&env)
        .args(["default", "nightly+opt"])
        .assert()
        .success();
}

#[test]
fn cached_catalog_survives_refresh_failure() {
    let env = TestEnv::new();
    let mirror = Mirror::new();
    mirror.command(&env).arg("list").assert().success();
    mirror.catalog_available.store(false, Ordering::SeqCst);
    mirror
        .command(&env)
        .args(["add", "nightly+opt"])
        .assert()
        .success()
        .stderr(predicate::str::contains("using the cached copy"));
    mirror
        .command(&env)
        .args(["add", "nightly+bogus"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Available variants: +opt"));
    mirror
        .command(&env)
        .args(["add", "1.9-nightly"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Available nightly channels: nightly",
        ));
}

#[test]
fn direct_download_requires_an_artifact_etag() {
    let env = TestEnv::new();
    let mirror = Mirror::new();
    mirror.etags.store(false, Ordering::SeqCst);
    mirror
        .command(&env)
        .args(["add", "nightly+opt"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("has no etag header"));
    assert!(
        !env.config_path().exists(),
        "failed install must not commit a channel"
    );
}

fn cache_path(env: &TestEnv) -> std::path::PathBuf {
    env.depot_path().join("juliaup/nightlies-cache.json")
}

#[test]
fn listing_reuses_fresh_metadata_and_installs_refresh() {
    let env = TestEnv::new();
    let mirror = Mirror::new();
    mirror.command(&env).arg("list").assert().success();
    mirror.command(&env).arg("list").assert().success();
    assert_eq!(mirror.payloads.load(Ordering::SeqCst), 1);
    mirror
        .command(&env)
        .args(["add", "nightly+opt"])
        .assert()
        .success();
    assert_eq!(mirror.payloads.load(Ordering::SeqCst), 2);
    let mut cache: serde_json::Value =
        serde_json::from_slice(&std::fs::read(cache_path(&env)).unwrap()).unwrap();
    cache["checked_at"] = "2000-01-01T00:00:00Z".into();
    std::fs::write(cache_path(&env), cache.to_string()).unwrap();
    *mirror.catalog.lock().unwrap() = CATALOG.replace("[\"opt\"]", "[\"future\"]");
    mirror
        .command(&env)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("nightly+future"));
    assert_eq!(mirror.payloads.load(Ordering::SeqCst), 3);
}

#[test]
fn malformed_refresh_preserves_cached_choices_and_release_listing() {
    let env = TestEnv::new();
    let mirror = Mirror::new();
    mirror.command(&env).arg("list").assert().success();
    let mut cache: serde_json::Value =
        serde_json::from_slice(&std::fs::read(cache_path(&env)).unwrap()).unwrap();
    cache["checked_at"] = "2000-01-01T00:00:00Z".into();
    let old = cache.to_string();
    std::fs::write(cache_path(&env), &old).unwrap();
    *mirror.catalog.lock().unwrap() = "not JSON".into();
    mirror
        .command(&env)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("release"))
        .stdout(predicate::str::contains("nightly+opt"));
    assert_eq!(std::fs::read_to_string(cache_path(&env)).unwrap(), old);
    std::fs::remove_file(cache_path(&env)).unwrap();
    mirror
        .command(&env)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("release"));
}

#[test]
fn variant_permutations_name_one_channel() {
    let env = TestEnv::new();
    let mirror = Mirror::new();
    *mirror.catalog.lock().unwrap() = CATALOG
        .replace("[\"opt\"]", "[\"opt\", \"assert\"]")
        .replace("https://julialangnightlies-s3.julialang.org", &mirror.url);
    mirror
        .command(&env)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nightly+assert+opt"));
    mirror
        .command(&env)
        .args(["add", "nightly+opt+assert"])
        .assert()
        .success();
    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("nightly+assert+opt"))
        .stdout(predicate::str::contains("nightly+opt+assert").not());
    let payloads = mirror.payloads.load(Ordering::SeqCst);
    let artifacts = mirror.artifacts.load(Ordering::SeqCst);
    mirror
        .command(&env)
        .args(["add", "nightly+assert+opt+opt"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "'nightly+assert+opt' is already installed.",
        ));
    assert_eq!(mirror.payloads.load(Ordering::SeqCst), payloads);
    assert_eq!(mirror.artifacts.load(Ordering::SeqCst), artifacts);
    env.julia()
        .args(["+nightly+opt+assert", "--version"])
        .assert()
        .success()
        .stdout("1.14.0-DEV.1");
    env.julia()
        .env("JULIAUP_CHANNEL", "nightly+opt+assert")
        .arg("--version")
        .assert()
        .success()
        .stdout("1.14.0-DEV.1");
    env.juliaup()
        .args(["link", "combined", "+nightly+opt+assert"])
        .assert()
        .success();
    env.julia()
        .args(["+combined", "--version"])
        .assert()
        .success()
        .stdout("1.14.0-DEV.1");
    env.juliaup()
        .args(["api", "getconfig1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("combined"));
    mirror.revision.store(2, Ordering::SeqCst);
    mirror
        .command(&env)
        .args(["update", "nightly+opt+assert"])
        .assert()
        .success();
    env.juliaup()
        .args(["override", "set", "nightly+opt+assert", "--path"])
        .arg(env.home_path())
        .assert()
        .success();
    env.julia()
        .current_dir(env.home_path())
        .arg("--version")
        .assert()
        .success()
        .stdout("1.14.0-DEV.2");
    env.juliaup()
        .args(["override", "unset", "--path"])
        .arg(env.home_path())
        .assert()
        .success();
    mirror
        .command(&env)
        .args(["default", "nightly+opt+assert"])
        .assert()
        .success();
    env.julia()
        .arg("--version")
        .assert()
        .success()
        .stdout("1.14.0-DEV.2");
    mirror
        .command(&env)
        .args(["remove", "nightly+opt+assert"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("default channel"));
    mirror
        .command(&env)
        .args(["add", "nightly+nogpl+opt"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("has no '+nogpl+opt' build"))
        .stderr(predicate::str::contains("Available variants: +assert+opt"));
}

#[test]
fn auto_install_variant_alias() {
    let env = TestEnv::new();
    let mirror = Mirror::new();
    *mirror.catalog.lock().unwrap() = CATALOG.replace("[\"opt\"]", "[\"opt\", \"assert\"]");
    env.juliaup()
        .args(["config", "autoinstallchannels", "true"])
        .assert()
        .success();
    env.juliaup()
        .args(["link", "nightly+z+a", "+nightly+opt+assert"])
        .assert()
        .success();
    env.julia()
        .env("JULIAUP_SERVER", &mirror.url)
        .env("JULIAUP_NIGHTLY_SERVER", &mirror.url)
        .args(["+nightly+z+a", "--version"])
        .assert()
        .success()
        .stdout("1.14.0-DEV.1");
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(env.config_path()).unwrap()).unwrap();
    let channels = config["InstalledChannels"].as_object().unwrap();
    assert_eq!(channels.len(), 2);
    assert!(channels.contains_key("nightly+assert+opt"));
    assert_eq!(channels["nightly+z+a"]["Target"], "nightly+assert+opt");
}

#[test]
fn linked_names_are_literal() {
    let env = TestEnv::new();
    for channel in ["custom+opt+assert", "nightly+opt+assert"] {
        env.juliaup()
            .args(["link", channel, "/bin/echo"])
            .assert()
            .success();
        env.julia()
            .args([&format!("+{channel}"), "linked"])
            .assert()
            .success()
            .stdout("linked\n");
        env.julia()
            .env("JULIAUP_CHANNEL", channel)
            .arg("linked")
            .assert()
            .success()
            .stdout("linked\n");
        env.juliaup()
            .args(["link", "alias", &format!("+{channel}")])
            .assert()
            .success();
        env.julia()
            .args(["+alias", "linked"])
            .assert()
            .success()
            .stdout("linked\n");
        env.juliaup().args(["default", channel]).assert().success();
        env.juliaup()
            .args(["remove", channel])
            .assert()
            .failure()
            .stderr(predicate::str::contains("default channel"));
        env.juliaup().args(["remove", "alias"]).assert().success();
    }
    env.juliaup()
        .args(["default", "custom+opt+assert"])
        .assert()
        .success();
    env.juliaup()
        .args(["remove", "nightly+opt+assert"])
        .assert()
        .success();
}

#[test]
fn release_only_updates_do_not_fetch_nightly_metadata() {
    let env = TestEnv::new();
    let mirror = Mirror::new();
    mirror.command(&env).arg("update").assert().success();
    assert_eq!(mirror.payloads.load(Ordering::SeqCst), 0);
}

#[test]
fn unsupported_pr_variants_never_trigger_auto_install() {
    let env = TestEnv::new();
    env.juliaup()
        .args(["config", "autoinstallchannels", "true"])
        .assert()
        .success();
    env.julia()
        .arg("+pr123+opt")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid Juliaup channel"))
        .stderr(predicate::str::contains("juliaup add pr123+opt").not())
        .stderr(predicate::str::contains("Installing").not());
    env.julia()
        .env("JULIAUP_CHANNEL", "pr123+opt")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid Juliaup channel"))
        .stderr(predicate::str::contains("juliaup add pr123+opt").not());
}

#[test]
fn manifest_without_metadata_launches_installed_series_nightly() {
    let env = TestEnv::new();
    let mirror = Mirror::new();
    // The bundled release database contains this series. Use an implausibly
    // high patch to exercise its unreleased-patch fallback.
    *mirror.catalog.lock().unwrap() = CATALOG
        .replace("\"nightly\"", "\"1.10-nightly\"")
        .replace("\"files\": [], \"variants\":", "\"files\":")
        .replace("[\"opt\"]", "[]");
    mirror
        .command(&env)
        .args(["add", "1.10-nightly"])
        .assert()
        .success();
    std::fs::remove_file(cache_path(&env)).unwrap();
    let project = env.home_path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("Project.toml"), "[deps]\n").unwrap();
    std::fs::write(
        project.join("Manifest.toml"),
        "julia_version = \"1.10.999\"\n",
    )
    .unwrap();
    env.juliaup()
        .args(["config", "manifestversiondetect", "true"])
        .assert()
        .success();
    env.julia()
        .current_dir(project)
        .arg("--version")
        .assert()
        .success()
        .stdout("1.14.0-DEV.1");
}

fn age_cache(env: &TestEnv) {
    let mut cache: serde_json::Value =
        serde_json::from_slice(&std::fs::read(cache_path(env)).unwrap()).unwrap();
    cache["checked_at"] = "2000-01-01T00:00:00Z".into();
    std::fs::write(cache_path(env), cache.to_string()).unwrap();
}

#[test]
fn changing_mirror_does_not_reuse_cached_choices() {
    let env = TestEnv::new();
    let first = Mirror::new();
    first.command(&env).arg("list").assert().success();
    let other = Mirror::new();
    other.catalog_available.store(false, Ordering::SeqCst);
    other
        .command(&env)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("nightly+opt").not());
    other
        .command(&env)
        .args(["add", "nightly+opt"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("using the cached copy").not());
    assert_eq!(other.payloads.load(Ordering::SeqCst), 2);
    // A failed request to the other source did not destroy the first cache.
    first
        .command(&env)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("nightly+opt"));
    assert_eq!(first.payloads.load(Ordering::SeqCst), 1);
}

#[test]
fn pr_links_and_aliases_do_not_trigger_catalog_downloads() {
    let env = TestEnv::new();
    let mirror = Mirror::new();
    // Obtain a real direct-download config, then reuse its installed artifact as
    // a PR channel. This avoids depending on live PR metadata for the gate test.
    mirror
        .command(&env)
        .args(["add", "nightly+opt"])
        .assert()
        .success();
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(env.config_path()).unwrap()).unwrap();
    let channels = config["InstalledChannels"].as_object_mut().unwrap();
    let nightly = channels.remove("nightly+opt").unwrap();
    channels.insert("pr123".into(), nightly);
    channels.insert(
        "nightly".into(),
        serde_json::json!({"Command": "/unused/julia", "Args": null}),
    );
    channels.insert(
        "nightly+opt".into(),
        serde_json::json!({"Target": "pr123", "Args": null}),
    );
    config["Default"] = "pr123".into();
    std::fs::write(env.config_path(), config.to_string()).unwrap();
    std::fs::remove_file(cache_path(&env)).unwrap();
    mirror.payloads.store(0, Ordering::SeqCst);
    mirror.command(&env).arg("update").assert().success();
    assert_eq!(mirror.payloads.load(Ordering::SeqCst), 0);
}
