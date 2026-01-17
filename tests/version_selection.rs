use indoc::indoc;
use juliaup::jsonstructs_versionsdb::{
    JuliaupVersionDB, JuliaupVersionDBChannel, JuliaupVersionDBVersion,
};
use juliaup::version_selection::{LOAD_PATH_SEPARATOR, *};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// Helper to create a test directory with Project.toml
fn create_test_project(dir: &Path, project_content: &str) -> PathBuf {
    let project_file = dir.join("Project.toml");
    fs::write(&project_file, project_content).unwrap();
    project_file
}

// Helper to create a manifest file
fn create_manifest(dir: &Path, name: &str, julia_version: &str) {
    let manifest_file = dir.join(name);
    fs::write(
        &manifest_file,
        format!(r#"julia_version = "{}""#, julia_version),
    )
    .unwrap();
}

// Helper to create a project with a standard manifest in one call
fn create_project_with_manifest(dir: &Path, julia_version: &str) -> PathBuf {
    let project_file = create_test_project(dir, "name = \"TestProject\"");
    create_manifest(dir, "Manifest.toml", julia_version);
    project_file
}

// Helper to build julia args, optionally with --project flag
// Pass None for no project flag, Some(path) for --project={path}
fn julia_args(project_path: Option<&Path>) -> Vec<String> {
    let mut args = vec!["julia".to_string()];
    if let Some(path) = project_path {
        args.push(format!("--project={}", path.display()));
    }
    args.push("-e".to_string());
    args.push("1+1".to_string());
    args
}

#[test]
fn test_load_path_expand_named_environment_dot() {
    // Test @. resolves to current directory's Project.toml
    let temp_dir = TempDir::new().unwrap();
    let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");

    let result = load_path_expand_impl("@.", temp_dir.path(), None).unwrap();

    assert!(result.is_some());
    // Canonicalize both paths for comparison (handles symlinks on macOS)
    assert_eq!(
        result.unwrap().canonicalize().unwrap(),
        project_file.canonicalize().unwrap()
    );
}

#[test]
fn test_load_path_expand_named_environment_depot() {
    // Test named environment like @v1.10 resolves to depot
    let temp_dir = TempDir::new().unwrap();
    let depot = temp_dir.path().join("depot");
    let env_dir = depot.join("environments").join("v1.10");
    fs::create_dir_all(&env_dir).unwrap();
    create_test_project(&env_dir, "name = \"TestEnv\"");

    let depot_path_str = depot.as_os_str();
    let result = load_path_expand_impl("@v1.10", temp_dir.path(), Some(depot_path_str)).unwrap();

    assert!(result.is_some());
    assert_eq!(result.unwrap(), env_dir.join("Project.toml"));
}

#[test]
fn test_load_path_expand_regular_path() {
    // Test regular path expansion
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("myproject");
    fs::create_dir(&project_dir).unwrap();
    let project_file = create_test_project(&project_dir, "name = \"MyProject\"");

    let result =
        load_path_expand_impl(project_dir.to_str().unwrap(), temp_dir.path(), None).unwrap();

    assert!(result.is_some());
    assert_eq!(result.unwrap(), project_file);
}

#[test]
fn test_determine_project_version_spec_from_env_var() {
    // Test JULIA_PROJECT environment variable auto-detects version
    // (Low-level test for JULIA_PROJECT parsing - complementary to end-to-end tests)
    let temp_dir = TempDir::new().unwrap();
    create_project_with_manifest(temp_dir.path(), "1.11.3");
    let args = julia_args(None);

    let result = determine_project_version_spec_impl(
        &args,
        Some(temp_dir.path().to_string_lossy().to_string()),
        None,
        temp_dir.path(),
    )
    .unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "1.11.3");
}

#[test]
fn test_determine_project_version_spec_from_env_var_empty_searches_upward() {
    // Test JULIA_PROJECT="" (empty) searches upward like @.
    let temp_dir = TempDir::new().unwrap();
    create_project_with_manifest(temp_dir.path(), "1.11.0");
    let args = julia_args(None);

    let result =
        determine_project_version_spec_impl(&args, Some("".to_string()), None, temp_dir.path())
            .unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "1.11.0");
}

#[test]
fn test_project_flag_stops_at_script_argument() {
    // Test that --project parsing stops at the first positional argument (script name)
    // julia script.jl --project=@foo should NOT use @foo
    let temp_dir = TempDir::new().unwrap();
    create_project_with_manifest(temp_dir.path(), "1.10.0");

    let args = vec![
        "julia".to_string(),
        "script.jl".to_string(),
        "--project=@foo".to_string(), // This should be ignored
    ];

    let result = determine_project_version_spec_impl(&args, None, None, temp_dir.path());
    // Should return None since --project=@foo comes after script.jl
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_project_flag_stops_at_double_dash() {
    // Test that --project parsing stops at --
    // julia -- script.jl --project=@foo should NOT use @foo
    let temp_dir = TempDir::new().unwrap();
    create_project_with_manifest(temp_dir.path(), "1.10.0");

    let args = vec![
        "julia".to_string(),
        "--".to_string(),
        "script.jl".to_string(),
        "--project=@foo".to_string(), // This should be ignored
    ];

    let result = determine_project_version_spec_impl(&args, None, None, temp_dir.path());
    // Should return None since --project=@foo comes after --
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_determine_project_version_spec_from_env_var_named_environment() {
    // Test JULIA_PROJECT=@. resolves to current directory
    let temp_dir = TempDir::new().unwrap();
    create_project_with_manifest(temp_dir.path(), "1.10.2");
    let args = julia_args(None);

    let result =
        determine_project_version_spec_impl(&args, Some("@.".to_string()), None, temp_dir.path())
            .unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "1.10.2");
}

#[test]
fn test_determine_project_version_spec_flag_overrides_env() {
    // Test that --project flag takes precedence over JULIA_PROJECT env var
    let temp_dir1 = TempDir::new().unwrap();
    create_test_project(temp_dir1.path(), "name = \"Project1\"");
    create_manifest(temp_dir1.path(), "Manifest.toml", "1.9.0");

    let temp_dir2 = TempDir::new().unwrap();
    create_test_project(temp_dir2.path(), "name = \"Project2\"");
    create_manifest(temp_dir2.path(), "Manifest.toml", "1.10.0");

    // But use --project to point to temp_dir2
    let args = vec![
        "julia".to_string(),
        format!("--project={}", temp_dir2.path().display()),
        "-e".to_string(),
        "1+1".to_string(),
    ];

    let result = determine_project_version_spec_impl(
        &args,
        Some(temp_dir1.path().to_string_lossy().to_string()), // JULIA_PROJECT=temp_dir1
        None,
        temp_dir2.path(),
    )
    .unwrap();
    assert!(result.is_some());
    // Should use the version from temp_dir2 (flag), not temp_dir1 (env)
    assert_eq!(result.unwrap(), "1.10.0");
}

#[test]
fn test_determine_project_version_spec_no_project_specified() {
    // Test that None is returned when no project is specified
    let temp_dir = TempDir::new().unwrap();
    let args = julia_args(None);

    let result = determine_project_version_spec_impl(&args, None, None, temp_dir.path()).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_determine_project_version_spec_from_load_path() {
    // Test JULIA_LOAD_PATH environment variable
    let temp_dir = TempDir::new().unwrap();
    create_project_with_manifest(temp_dir.path(), "1.12.0");

    let load_path = format!(
        "@{}{}{}@stdlib",
        LOAD_PATH_SEPARATOR,
        temp_dir.path().display(),
        LOAD_PATH_SEPARATOR
    );
    let args = julia_args(None);

    let result =
        determine_project_version_spec_impl(&args, None, Some(load_path), temp_dir.path()).unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "1.12.0");
}

#[test]
fn test_determine_project_version_spec_load_path_searches_first_valid() {
    // Test that JULIA_LOAD_PATH returns the first valid project
    let temp_dir1 = TempDir::new().unwrap();
    create_test_project(temp_dir1.path(), "name = \"Project1\"");
    create_manifest(temp_dir1.path(), "Manifest.toml", "1.11.5");

    let temp_dir2 = TempDir::new().unwrap();
    create_test_project(temp_dir2.path(), "name = \"Project2\"");
    create_manifest(temp_dir2.path(), "Manifest.toml", "1.10.3");

    let load_path = format!(
        "{}{}{}",
        temp_dir1.path().display(),
        LOAD_PATH_SEPARATOR,
        temp_dir2.path().display()
    );

    let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

    let result =
        determine_project_version_spec_impl(&args, None, Some(load_path), temp_dir1.path())
            .unwrap();
    assert!(result.is_some());
    // Should use version from temp_dir1 (first in LOAD_PATH)
    assert_eq!(result.unwrap(), "1.11.5");
}

#[test]
fn test_determine_project_version_spec_project_overrides_load_path() {
    // Test that JULIA_PROJECT takes precedence over JULIA_LOAD_PATH
    let temp_dir1 = TempDir::new().unwrap();
    create_test_project(temp_dir1.path(), "name = \"Project1\"");
    create_manifest(temp_dir1.path(), "Manifest.toml", "1.9.2");

    let temp_dir2 = TempDir::new().unwrap();
    create_test_project(temp_dir2.path(), "name = \"Project2\"");
    create_manifest(temp_dir2.path(), "Manifest.toml", "1.10.4");

    let args = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];

    let result = determine_project_version_spec_impl(
        &args,
        Some(temp_dir2.path().to_string_lossy().to_string()), // JULIA_PROJECT
        Some(temp_dir1.path().to_string_lossy().to_string()), // JULIA_LOAD_PATH
        temp_dir2.path(),
    )
    .unwrap();
    assert!(result.is_some());
    // Should use version from temp_dir2 (JULIA_PROJECT), not temp_dir1 (JULIA_LOAD_PATH)
    assert_eq!(result.unwrap(), "1.10.4");
}

#[test]
fn test_determine_project_version_spec_relative_paths() {
    // Test that relative paths work like Julia (relative to current directory)
    // Similar to: JULIA_LOAD_PATH="Pkg.jl" julia --project=DataFrames.jl -e '...'
    let parent_dir = TempDir::new().unwrap();

    // Create two project directories
    let pkg_dir = parent_dir.path().join("Pkg.jl");
    fs::create_dir(&pkg_dir).unwrap();
    create_test_project(&pkg_dir, "name = \"Pkg\"");
    create_manifest(&pkg_dir, "Manifest.toml", "1.9.0");

    let df_dir = parent_dir.path().join("DataFrames.jl");
    fs::create_dir(&df_dir).unwrap();
    create_test_project(&df_dir, "name = \"DataFrames\"");
    create_manifest(&df_dir, "Manifest.toml", "1.11.0");

    // Test 1: --project=DataFrames.jl should override JULIA_LOAD_PATH
    let args = vec![
        "julia".to_string(),
        "--project=DataFrames.jl".to_string(),
        "-e".to_string(),
        "1+1".to_string(),
    ];
    let result = determine_project_version_spec_impl(
        &args,
        None,
        Some("Pkg.jl".to_string()),
        parent_dir.path(),
    )
    .unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "1.11.0");

    // Test 2: Without --project, should use JULIA_LOAD_PATH (Pkg.jl)
    let args2 = vec!["julia".to_string(), "-e".to_string(), "1+1".to_string()];
    let result2 = determine_project_version_spec_impl(
        &args2,
        None,
        Some("Pkg.jl".to_string()),
        parent_dir.path(),
    )
    .unwrap();
    assert!(result2.is_some());
    assert_eq!(result2.unwrap(), "1.9.0");
}

#[test]
fn test_project_parsing_with_required_arg_options() {
    // Test that --project parsing correctly handles options with required arguments (getopt behavior)
    // E.g., "julia --module --project foo.jl" treats "--project" as the module name, not a flag
    let temp_dir = TempDir::new().unwrap();
    create_project_with_manifest(temp_dir.path(), "1.10.5");

    let to_args = |parts: &[&str]| parts.iter().map(|s| s.to_string()).collect::<Vec<_>>();

    // --project consumed by --module (long option)
    assert!(determine_project_version_spec_impl(
        &to_args(&["julia", "--module", "--project", "foo.jl"]),
        None,
        None,
        temp_dir.path()
    )
    .unwrap()
    .is_none());

    // --project consumed by -e (short option)
    assert!(determine_project_version_spec_impl(
        &to_args(&["julia", "-e", "--project", "foo.jl"]),
        None,
        None,
        temp_dir.path()
    )
    .unwrap()
    .is_none());

    // --project after multiple required args still works
    assert_eq!(
        determine_project_version_spec_impl(
            &to_args(&[
                "julia",
                "--eval",
                "1+1",
                &format!("--project={}", temp_dir.path().display())
            ]),
            None,
            None,
            temp_dir.path()
        )
        .unwrap(),
        Some("1.10.5".to_string())
    );

    // Options with = don't consume next token
    assert_eq!(
        determine_project_version_spec_impl(
            &to_args(&[
                "julia",
                "--eval=1+1",
                &format!("--project={}", temp_dir.path().display())
            ]),
            None,
            None,
            temp_dir.path()
        )
        .unwrap(),
        Some("1.10.5".to_string())
    );
}

#[test]
fn test_current_project_direct_search() {
    // Test current_project finds Project.toml in directory
    let temp_dir = TempDir::new().unwrap();
    let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");

    let result = current_project(temp_dir.path());
    assert!(result.is_some());
    assert_eq!(result.unwrap(), project_file);
}

#[test]
fn test_current_project_search_upward() {
    // Test current_project searches upward for Project.toml
    let temp_dir = TempDir::new().unwrap();
    let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");

    // Create a subdirectory
    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();

    let result = current_project(&subdir);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), project_file);
}

#[test]
fn test_current_project_julia_project_precedence() {
    // Test that JuliaProject.toml takes precedence over Project.toml
    let temp_dir = TempDir::new().unwrap();
    create_test_project(temp_dir.path(), "name = \"TestProject\"");
    let julia_project_file = temp_dir.path().join("JuliaProject.toml");
    fs::write(&julia_project_file, "name = \"JuliaTestProject\"").unwrap();

    let result = current_project(temp_dir.path());
    assert!(result.is_some());
    assert_eq!(result.unwrap(), julia_project_file);
}

#[test]
fn test_current_project_not_found() {
    // Test when no project file exists
    let temp_dir = TempDir::new().unwrap();
    let result = current_project(temp_dir.path());
    assert!(result.is_none());
}

#[test]
fn test_project_file_manifest_path_default() {
    // Test default Manifest.toml detection
    let temp_dir = TempDir::new().unwrap();
    let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
    create_manifest(temp_dir.path(), "Manifest.toml", "1.10.0");

    let result = project_file_manifest_path(&project_file);

    assert!(result.is_some());
    assert_eq!(result.unwrap().file_name().unwrap(), "Manifest.toml");
}

#[test]
fn test_project_file_manifest_path_julia_manifest_precedence() {
    // Test that JuliaManifest.toml takes precedence over Manifest.toml
    let temp_dir = TempDir::new().unwrap();
    let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
    create_manifest(temp_dir.path(), "Manifest.toml", "1.10.0");
    create_manifest(temp_dir.path(), "JuliaManifest.toml", "1.11.0");

    let result = project_file_manifest_path(&project_file);

    assert!(result.is_some());
    assert_eq!(result.unwrap().file_name().unwrap(), "JuliaManifest.toml");
}

#[test]
fn test_project_file_manifest_path_versioned_manifest() {
    // Test versioned manifest detection
    let temp_dir = TempDir::new().unwrap();
    let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
    create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");

    let result = project_file_manifest_path(&project_file);

    assert!(result.is_some());
    assert_eq!(result.unwrap().file_name().unwrap(), "Manifest-v1.11.toml");
}

#[test]
fn test_determine_manifest_path_multiple_versioned_manifests() {
    // Test that the highest versioned manifest is selected
    let temp_dir = TempDir::new().unwrap();
    let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
    create_manifest(temp_dir.path(), "Manifest-v1.10.toml", "1.10.0");
    create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");
    create_manifest(temp_dir.path(), "Manifest-v1.12.toml", "1.12.0");

    let result = project_file_manifest_path(&project_file);

    assert!(result.is_some());
    assert_eq!(result.unwrap().file_name().unwrap(), "Manifest-v1.12.toml");
}

#[test]
fn test_versioned_manifest_priority_over_standard() {
    // Test that versioned manifests take precedence over standard manifests
    let temp_dir = TempDir::new().unwrap();
    let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
    create_manifest(temp_dir.path(), "Manifest.toml", "1.13.0");
    create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");
    create_manifest(temp_dir.path(), "Manifest-v1.12.toml", "1.12.0");

    let result = project_file_manifest_path(&project_file);

    assert!(result.is_some());
    // Versioned manifest should be selected (highest version)
    assert_eq!(result.unwrap().file_name().unwrap(), "Manifest-v1.12.toml");
}

#[test]
fn test_julia_manifest_priority_over_manifest() {
    // Test that JuliaManifest-v*.toml takes precedence over Manifest-v*.toml for same version
    let temp_dir = TempDir::new().unwrap();
    let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
    create_manifest(temp_dir.path(), "JuliaManifest-v1.11.toml", "1.11.0");
    create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");

    let result = project_file_manifest_path(&project_file);

    assert!(result.is_some());
    assert_eq!(
        result.unwrap().file_name().unwrap(),
        "JuliaManifest-v1.11.toml"
    );
}

#[test]
fn test_higher_version_wins_regardless_of_prefix() {
    // Test that higher version wins even if it's Manifest (not JuliaManifest)
    let temp_dir = TempDir::new().unwrap();
    let project_file = create_test_project(temp_dir.path(), "name = \"TestProject\"");
    create_manifest(temp_dir.path(), "JuliaManifest-v1.10.toml", "1.10.0");
    create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");

    let result = project_file_manifest_path(&project_file);

    assert!(result.is_some());
    assert_eq!(result.unwrap().file_name().unwrap(), "Manifest-v1.11.toml");
}

#[test]
fn test_determine_manifest_path_explicit_manifest_field() {
    // Test explicit manifest field in Project.toml
    let temp_dir = TempDir::new().unwrap();
    let project_file = create_test_project(
        temp_dir.path(),
        indoc! {r#"
            name = "TestProject"
            manifest = "custom/Manifest.toml"
        "#},
    );

    let custom_dir = temp_dir.path().join("custom");
    fs::create_dir(&custom_dir).unwrap();
    create_manifest(&custom_dir, "Manifest.toml", "1.10.0");

    let result = project_file_manifest_path(&project_file);

    assert!(result.is_some());
    assert!(result.unwrap().ends_with("custom/Manifest.toml"));
}

#[test]
fn test_read_manifest_julia_version() {
    // Test reading julia_version from manifest
    let temp_dir = TempDir::new().unwrap();
    create_manifest(temp_dir.path(), "Manifest.toml", "1.10.5");

    let manifest_path = temp_dir.path().join("Manifest.toml");
    let result = read_manifest_julia_version(&manifest_path).unwrap();

    assert!(result.is_some());
    assert_eq!(result.unwrap(), "1.10.5");
}

#[test]
fn test_read_manifest_julia_version_missing_file() {
    // Test reading from non-existent manifest
    let temp_dir = TempDir::new().unwrap();
    let manifest_path = temp_dir.path().join("NonExistent.toml");

    let result = read_manifest_julia_version(&manifest_path).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_read_manifest_julia_version_missing_field() {
    // Test reading manifest without julia_version field
    let temp_dir = TempDir::new().unwrap();
    let manifest_path = temp_dir.path().join("Manifest.toml");
    fs::write(&manifest_path, "[deps]\nExample = \"1.0.0\"").unwrap();

    let result = read_manifest_julia_version(&manifest_path).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_find_highest_versioned_manifest() {
    // Test finding highest versioned manifest
    let temp_dir = TempDir::new().unwrap();
    create_manifest(temp_dir.path(), "Manifest-v1.8.0.toml", "1.8.0");
    create_manifest(temp_dir.path(), "Manifest-v1.10.5.toml", "1.10.5");
    create_manifest(temp_dir.path(), "Manifest-v1.11.2.toml", "1.11.2");

    let result = find_highest_versioned_manifest(temp_dir.path());
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().file_name().unwrap(),
        "Manifest-v1.11.2.toml"
    );
}

#[test]
fn test_find_highest_versioned_manifest_none() {
    // Test when no versioned manifests exist
    let temp_dir = TempDir::new().unwrap();
    create_manifest(temp_dir.path(), "Manifest.toml", "1.10.0");

    let result = find_highest_versioned_manifest(temp_dir.path());
    assert!(result.is_none());
}

#[test]
fn test_find_highest_versioned_manifest_invalid_names() {
    // Test that invalid versioned manifest names are ignored
    let temp_dir = TempDir::new().unwrap();
    create_manifest(temp_dir.path(), "Manifest-v1.11.toml", "1.11.0");
    fs::write(temp_dir.path().join("Manifest-vInvalid.toml"), "invalid").unwrap();
    fs::write(temp_dir.path().join("Manifest-v.toml"), "invalid").unwrap();

    let result = find_highest_versioned_manifest(temp_dir.path());
    assert!(result.is_some());
    assert_eq!(result.unwrap().file_name().unwrap(), "Manifest-v1.11.toml");
}

#[test]
fn test_resolve_auto_channel_high_patch_version() {
    // Test that a patch version higher than any known minor version uses X.Y-nightly
    let versions_db = TestVersionsDbBuilder::new()
        .add_version("1.12.0")
        .add_channel("1.12.0", "1.12.0")
        .add_version("1.12.1")
        .add_channel("1.12.1", "1.12.1")
        .build();

    // Test 1: Version 1.12.55 (higher patch than any known) should resolve to 1.12-nightly
    let result = resolve_auto_channel("1.12.55".to_string(), &versions_db);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "1.12-nightly");

    // Test 2: Version 1.12.1 (exact match) should resolve to itself
    let result = resolve_auto_channel("1.12.1".to_string(), &versions_db);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "1.12.1");

    // Test 3: Version 1.12.0 (exact match) should resolve to itself
    let result = resolve_auto_channel("1.12.0".to_string(), &versions_db);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "1.12.0");
}

#[test]
fn test_resolve_auto_channel_higher_than_any_version() {
    // Test that a version higher than any known version uses nightly
    let versions_db = TestVersionsDbBuilder::new()
        .add_version("1.12.0")
        .add_channel("1.12.0", "1.12.0")
        .add_version("1.12.1")
        .add_channel("1.12.1", "1.12.1")
        .build();

    // Version 1.13.0 (higher than any known version) should resolve to nightly
    let result = resolve_auto_channel("1.13.0".to_string(), &versions_db);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "nightly");
}

#[test]
fn test_resolve_auto_channel_prerelease_versions() {
    // Test that prerelease versions use nightly channels appropriately
    let versions_db = TestVersionsDbBuilder::new()
        .add_version("1.11.0")
        .add_channel("1.11.0", "1.11.0")
        .add_version("1.12.1")
        .add_channel("1.12.1", "1.12.1")
        .add_channel("1.12.0-rc1", "1.12.0-rc1")
        .add_channel("1.12-nightly", "1.12.2-DEV")
        .add_channel("1.13-nightly", "1.13.0-DEV")
        .build();

    // Test 1: Exact match - 1.12.0-rc1 exists, so use it
    let result = resolve_auto_channel("1.12.0-rc1".to_string(), &versions_db);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "1.12.0-rc1");

    // Test 2: Exact match for prerelease of existing stable - 1.12.1-rc1 exists, so use it
    // Add 1.12.1-rc1 channel to test this case
    let versions_db_with_rc = TestVersionsDbBuilder::new()
        .add_version("1.11.0")
        .add_channel("1.11.0", "1.11.0")
        .add_version("1.12.1")
        .add_channel("1.12.1", "1.12.1")
        .add_channel("1.12.1-rc1", "1.12.1-rc1") // Prerelease of stable version
        .add_channel("1.12-nightly", "1.12.2-DEV")
        .build();

    let result = resolve_auto_channel("1.12.1-rc1".to_string(), &versions_db_with_rc);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "1.12.1-rc1");

    // Test 3: CRITICAL - 1.12.1-DEV < 1.12.1 in SemVer ordering, but should still use nightly
    // This is the common case when a manifest is generated on nightly
    let result = resolve_auto_channel("1.12.1-DEV".to_string(), &versions_db);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "1.12-nightly");

    // Test 4: 1.13.0-DEV should use 1.13-nightly
    let result = resolve_auto_channel("1.13.0-DEV".to_string(), &versions_db);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "1.13-nightly");

    // Test 5: 1.14.0-DEV (no 1.14-nightly exists), should use main nightly
    let result = resolve_auto_channel("1.14.0-DEV".to_string(), &versions_db);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "nightly");
}

// Helper to build a test versions database
struct TestVersionsDbBuilder {
    available_versions: HashMap<String, JuliaupVersionDBVersion>,
    available_channels: HashMap<String, JuliaupVersionDBChannel>,
}

impl TestVersionsDbBuilder {
    fn new() -> Self {
        Self {
            available_versions: HashMap::new(),
            available_channels: HashMap::new(),
        }
    }

    fn add_version(mut self, version: &str) -> Self {
        self.available_versions.insert(
            version.to_string(),
            JuliaupVersionDBVersion {
                url_path: "test".to_string(),
            },
        );
        self
    }

    fn add_channel(mut self, channel: &str, version: &str) -> Self {
        self.available_channels.insert(
            channel.to_string(),
            JuliaupVersionDBChannel {
                version: version.to_string(),
            },
        );
        self
    }

    fn build(self) -> JuliaupVersionDB {
        JuliaupVersionDB {
            available_versions: self.available_versions,
            available_channels: self.available_channels,
            version: "1".to_string(),
        }
    }
}
