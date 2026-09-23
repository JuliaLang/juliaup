use juliaup::julia_compat::{compute_upgrade_offer, UpgradeOffer, VersionSpec};
use semver::Version;

const VERSIONS: &[&str] = &[
    "0.0.3", "0.0.4", "0.5.1", "0.6.0", "1.0.0", "1.5.9", "1.6.0", "1.9.3", "1.9.4", "1.10.2",
    "1.10.3", "1.10.4", "1.11.0", "1.12.1", "2.0.0",
];

#[test]
fn test_version_spec_matches_pkg() {
    // Truth table generated with Pkg's `semver_spec`: for each specifier, the
    // versions from VERSIONS it contains
    let cases = [
        (
            "1.6",
            "1.6.0 1.9.3 1.9.4 1.10.2 1.10.3 1.10.4 1.11.0 1.12.1",
        ),
        ("1.10, 1.11", "1.10.2 1.10.3 1.10.4 1.11.0 1.12.1"),
        ("~1.10, ~1.11", "1.10.2 1.10.3 1.10.4 1.11.0"),
        ("^1.10.2", "1.10.2 1.10.3 1.10.4 1.11.0 1.12.1"),
        ("~1.10.2", "1.10.2 1.10.3 1.10.4"),
        (
            "~1",
            "1.0.0 1.5.9 1.6.0 1.9.3 1.9.4 1.10.2 1.10.3 1.10.4 1.11.0 1.12.1",
        ),
        (
            "1",
            "1.0.0 1.5.9 1.6.0 1.9.3 1.9.4 1.10.2 1.10.3 1.10.4 1.11.0 1.12.1",
        ),
        ("0.5", "0.5.1"),
        ("0.0.3", "0.0.3"),
        ("=1.10.4", "1.10.4"),
        (
            ">= 1.9",
            "1.9.3 1.9.4 1.10.2 1.10.3 1.10.4 1.11.0 1.12.1 2.0.0",
        ),
        (
            "≥1.9.1",
            "1.9.3 1.9.4 1.10.2 1.10.3 1.10.4 1.11.0 1.12.1 2.0.0",
        ),
        (
            "< 1.11",
            "0.0.3 0.0.4 0.5.1 0.6.0 1.0.0 1.5.9 1.6.0 1.9.3 1.9.4 1.10.2 1.10.3 1.10.4",
        ),
        (
            "<1.10.3",
            "0.0.3 0.0.4 0.5.1 0.6.0 1.0.0 1.5.9 1.6.0 1.9.3 1.9.4 1.10.2",
        ),
        ("1.6 - 1.9", "1.6.0 1.9.3 1.9.4"),
        ("1.6.2 - 1.9.3", "1.9.3"),
        (
            "1 - 1",
            "1.0.0 1.5.9 1.6.0 1.9.3 1.9.4 1.10.2 1.10.3 1.10.4 1.11.0 1.12.1",
        ),
        ("v1.8", "1.9.3 1.9.4 1.10.2 1.10.3 1.10.4 1.11.0 1.12.1"),
        ("0.0", "0.0.3 0.0.4"),
        ("0", "0.0.3 0.0.4 0.5.1 0.6.0"),
    ];

    for (spec, expected) in cases {
        let parsed = VersionSpec::parse(spec).unwrap();
        let contained: Vec<&str> = VERSIONS
            .iter()
            .copied()
            .filter(|v| parsed.contains(&Version::parse(v).unwrap()))
            .collect();
        assert_eq!(contained.join(" "), expected, "spec `{}`", spec);
    }

    for invalid in ["1.x", "abc", "0.0.0", "=1.2.3.4", "<= 1.2"] {
        assert!(VersionSpec::parse(invalid).is_err(), "spec `{}`", invalid);
    }
}

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

fn available() -> Vec<Version> {
    [
        "1.9.4", "1.10.0", "1.10.4", "1.10.12", "1.11.0", "1.11.7", "1.12.1",
    ]
    .iter()
    .map(|s| v(s))
    .collect()
}

fn specs(entries: &[&str]) -> Vec<VersionSpec> {
    entries
        .iter()
        .map(|s| VersionSpec::parse(s).unwrap())
        .collect()
}

#[test]
fn test_upgrade_offer() {
    // Caret compat allows the newest release; the patch upgrade is offered separately
    assert_eq!(
        compute_upgrade_offer(&v("1.10.4"), &specs(&["1.10"]), &available()),
        Some(UpgradeOffer {
            latest: v("1.12.1"),
            latest_patch: Some(v("1.10.12")),
            current_violates_compat: false,
        })
    );

    // Restricted compat
    assert_eq!(
        compute_upgrade_offer(&v("1.10.4"), &specs(&["~1.10, ~1.11"]), &available()),
        Some(UpgradeOffer {
            latest: v("1.11.7"),
            latest_patch: Some(v("1.10.12")),
            current_violates_compat: false,
        })
    );

    // Only patch upgrades allowed: no separate patch option
    assert_eq!(
        compute_upgrade_offer(&v("1.10.4"), &specs(&["~1.10"]), &available()),
        Some(UpgradeOffer {
            latest: v("1.10.12"),
            latest_patch: None,
            current_violates_compat: false,
        })
    );

    // No compat entry: patch upgrades only
    assert_eq!(
        compute_upgrade_offer(&v("1.10.4"), &[], &available()),
        Some(UpgradeOffer {
            latest: v("1.10.12"),
            latest_patch: None,
            current_violates_compat: false,
        })
    );

    // Already on the newest allowed version, or pinned
    assert_eq!(
        compute_upgrade_offer(&v("1.12.1"), &specs(&["1.10"]), &available()),
        None
    );
    assert_eq!(
        compute_upgrade_offer(&v("1.10.4"), &specs(&["=1.10.4"]), &available()),
        None
    );

    // All workspace compat entries must be satisfied
    assert_eq!(
        compute_upgrade_offer(
            &v("1.10.4"),
            &specs(&["1.10", "~1.10, ~1.11"]),
            &available()
        ),
        Some(UpgradeOffer {
            latest: v("1.11.7"),
            latest_patch: Some(v("1.10.12")),
            current_violates_compat: false,
        })
    );

    // The manifest version violates compat
    assert_eq!(
        compute_upgrade_offer(&v("1.10.4"), &specs(&["1.11"]), &available()),
        Some(UpgradeOffer {
            latest: v("1.12.1"),
            latest_patch: None,
            current_violates_compat: true,
        })
    );
    // ... even if that means moving to an older version
    assert_eq!(
        compute_upgrade_offer(&v("1.10.4"), &specs(&["=1.10.0"]), &available()),
        Some(UpgradeOffer {
            latest: v("1.10.0"),
            latest_patch: None,
            current_violates_compat: true,
        })
    );

    // Nothing available that satisfies compat
    assert_eq!(
        compute_upgrade_offer(&v("1.10.4"), &specs(&["2"]), &available()),
        None
    );
}
