use indoc::indoc;
use juliaup::project_instantiation::{
    check_instantiation, depot_paths, version_slug, InstantiationNeed,
};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_version_slug_matches_julia() {
    // Golden values computed with Julia's `Base.version_slug(uuid, sha1)` and
    // `Base.version_slug(uuid, sha1, 4)`
    let cases = [
        (
            "7876af07-990d-54b4-ab0e-23690620f79a",
            "46e44e869b4d90b96bd8ed1fdcf32244fddfb6cc",
            "aqsx3",
            "aqsx",
        ),
        (
            "00000000-0000-0000-0000-000000000001",
            "0000000000000000000000000000000000000000",
            "cfAQr",
            "cfAQ",
        ),
        (
            "ffffffff-ffff-ffff-ffff-ffffffffffff",
            "ffffffffffffffffffffffffffffffffffffffff",
            "ULo9a",
            "ULo9",
        ),
        (
            "621f4979-c628-5d54-868e-fcf4e3e8185c",
            "d92ad398961a3ed262d8bf04a1a2b8340f915fef",
            "4iQz5",
            "4iQz",
        ),
    ];

    for (uuid, sha1, slug5, slug4) in cases {
        assert_eq!(version_slug(uuid, sha1, 5).unwrap(), slug5);
        assert_eq!(version_slug(uuid, sha1, 4).unwrap(), slug4);
    }

    assert!(version_slug("not-a-uuid", "46e44e869b4d90b96bd8ed1fdcf32244fddfb6cc", 5).is_err());
    assert!(version_slug("7876af07-990d-54b4-ab0e-23690620f79a", "abc", 5).is_err());
}

#[test]
fn test_depot_paths() {
    let home_depot = dirs::home_dir().unwrap().join(".julia");
    let julia_bin = PathBuf::from("/opt/julia/bin/julia");

    // Default depots
    let depots = depot_paths(None, Some(&julia_bin));
    assert_eq!(depots[0], home_depot);
    assert_eq!(depots[1], PathBuf::from("/opt/julia/local/share/julia"));
    assert_eq!(depots[2], PathBuf::from("/opt/julia/share/julia"));

    // Explicit depots
    let explicit = std::env::join_paths(["/a", "/b"]).unwrap();
    let depots = depot_paths(Some(&explicit), Some(&julia_bin));
    assert_eq!(depots, vec![PathBuf::from("/a"), PathBuf::from("/b")]);

    // An empty entry expands to the default depots
    let with_empty = std::env::join_paths(["/a", ""]).unwrap();
    let depots = depot_paths(Some(&with_empty), None);
    assert_eq!(depots, vec![PathBuf::from("/a"), home_depot]);
}

const EXAMPLE_UUID: &str = "7876af07-990d-54b4-ab0e-23690620f79a";
const EXAMPLE_SHA1: &str = "46e44e869b4d90b96bd8ed1fdcf32244fddfb6cc";

fn write_manifest(dir: &std::path::Path) -> PathBuf {
    let manifest = dir.join("Manifest.toml");
    fs::write(
        &manifest,
        format!(
            indoc! {r#"
                julia_version = "1.11.2"
                manifest_format = "2.0"

                [[deps.Example]]
                git-tree-sha1 = "{}"
                uuid = "{}"
                version = "0.5.5"

                [[deps.Dev]]
                path = "dev/Dev"
                uuid = "00000000-0000-0000-0000-000000000002"

                [[deps.Random]]
                uuid = "9a3f8284-a2c9-5f02-9a11-845980a1fd5c"
                version = "1.11.0"
            "#},
            EXAMPLE_SHA1, EXAMPLE_UUID
        ),
    )
    .unwrap();
    manifest
}

#[test]
fn test_check_instantiation() {
    let project = TempDir::new().unwrap();
    let depot = TempDir::new().unwrap();
    let depots = vec![depot.path().to_path_buf()];
    let manifest = write_manifest(project.path());

    // Nothing installed yet (stdlibs don't count)
    assert_eq!(
        check_instantiation(Some(&manifest), true, &depots).unwrap(),
        Some(InstantiationNeed::MissingPackages(vec![
            "Dev".to_string(),
            "Example".to_string()
        ]))
    );

    // Install the packages
    fs::create_dir_all(project.path().join("dev").join("Dev")).unwrap();
    let slug = version_slug(EXAMPLE_UUID, EXAMPLE_SHA1, 5).unwrap();
    fs::create_dir_all(depot.path().join("packages").join("Example").join(slug)).unwrap();
    assert_eq!(
        check_instantiation(Some(&manifest), true, &depots).unwrap(),
        None
    );

    // Packages installed under the old 4-character slug are found too
    let depot4 = TempDir::new().unwrap();
    fs::create_dir_all(
        depot4
            .path()
            .join("packages")
            .join("Example")
            .join(version_slug(EXAMPLE_UUID, EXAMPLE_SHA1, 4).unwrap()),
    )
    .unwrap();
    assert_eq!(
        check_instantiation(Some(&manifest), true, &[depot4.path().to_path_buf()]).unwrap(),
        None
    );
}

#[test]
fn test_check_instantiation_without_manifest() {
    assert_eq!(
        check_instantiation(None, true, &[]).unwrap(),
        Some(InstantiationNeed::NoManifest)
    );
    assert_eq!(check_instantiation(None, false, &[]).unwrap(), None);
}

#[test]
fn test_check_instantiation_manifest_format_1() {
    let project = TempDir::new().unwrap();
    let manifest = project.path().join("Manifest.toml");
    fs::write(
        &manifest,
        format!(
            "[[Example]]\ngit-tree-sha1 = \"{}\"\nuuid = \"{}\"\n",
            EXAMPLE_SHA1, EXAMPLE_UUID
        ),
    )
    .unwrap();
    assert_eq!(
        check_instantiation(Some(&manifest), true, &[]).unwrap(),
        Some(InstantiationNeed::MissingPackages(vec![
            "Example".to_string()
        ]))
    );
}
