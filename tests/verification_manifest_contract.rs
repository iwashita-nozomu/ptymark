// @dependency-start
// contract test
// responsibility Validates the verification catalog and referenced paths.
// upstream design ../verification/manifest.toml required checks
// upstream design ../verification/README.md evidence policy
// downstream environment ../.github/workflows/ptymark-ci.yml supported-platform execution
// @dependency-end

use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: u32,
    catalog: String,
    policy: Policy,
    check: Vec<Check>,
}

#[derive(Debug, Deserialize)]
struct Policy {
    all_required: bool,
    run_results_live_in: String,
    canonical_product_workflow: String,
    repository_workflow: String,
    docker_workflow: String,
    agent_workflow: String,
    compatibility_inventory: String,
}

#[derive(Debug, Deserialize)]
struct Check {
    id: String,
    owner: String,
    level: String,
    platforms: Vec<String>,
    command: String,
    sources: Vec<String>,
    evidence: Vec<String>,
    required: bool,
}

const REQUIRED_IDS: &[&str] = &[
    "catalog.schema",
    "rust.format",
    "rust.clippy",
    "rust.tests",
    "terminal.byte-exact",
    "terminal.alternate-screen",
    "detector.explicit-fences",
    "pipeline.single-commit",
    "runtime.pipeline-factory",
    "runtime.stream-pump",
    "command.filtered-run",
    "pty.native-session",
    "routing.typed-handoff",
    "cache.contract",
    "config.contract",
    "engine.executable-resolution",
    "engine.user-installed",
    "engine.managed-docker",
    "engine.managed-windows",
    "installer.unix",
    "installer.powershell",
    "installer.cmd",
    "installer.winbash",
    "shell.inventory",
    "shell.behavior-profiles",
    "shell.profile-coexistence-unix",
    "shell.profile-coexistence-windows",
    "wezterm.plugin",
    "scripts.syntax",
    "package-smoke.linux",
    "package-smoke.macos",
    "package-smoke.windows",
    "repository.ci",
    "repository.docker-build",
    "repository.agent-improvement-guide",
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn load_manifest() -> Manifest {
    let path = repository_root().join("verification/manifest.toml");
    let source = fs::read_to_string(&path).expect("read verification manifest");
    toml::from_str(&source).expect("parse verification manifest")
}

fn assert_repository_path(root: &Path, relative: &str) {
    let path = Path::new(relative);
    assert!(
        !path.is_absolute(),
        "catalog path must be repository-relative: {relative}"
    );
    assert!(
        root.join(path).exists(),
        "catalog path does not exist: {relative}"
    );
}

#[test]
fn verification_catalog_is_complete_and_self_consistent() {
    let root = repository_root();
    let manifest = load_manifest();
    let documentation = fs::read_to_string(root.join("verification/README.md"))
        .expect("read verification documentation");

    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.catalog, "ptymark-merge-gates");
    assert!(manifest.policy.all_required);
    assert!(!manifest.policy.run_results_live_in.trim().is_empty());

    for path in [
        &manifest.policy.canonical_product_workflow,
        &manifest.policy.repository_workflow,
        &manifest.policy.docker_workflow,
        &manifest.policy.agent_workflow,
        &manifest.policy.compatibility_inventory,
    ] {
        assert_repository_path(&root, path);
    }

    let allowed_owners = HashSet::from([
        "ptymark-ci",
        "repository-ci",
        "docker-build",
        "agent-improvement-guide",
    ]);
    let allowed_levels = HashSet::from([
        "static",
        "contract",
        "integration",
        "package",
        "compatibility",
        "repository",
    ]);
    let allowed_platforms = HashSet::from(["linux", "macos", "windows", "docker", "repository"]);

    let mut ids = HashSet::new();
    for check in &manifest.check {
        assert!(!check.id.trim().is_empty());
        assert!(
            ids.insert(check.id.as_str()),
            "duplicate check ID: {}",
            check.id
        );
        assert!(
            allowed_owners.contains(check.owner.as_str()),
            "unknown owner for {}: {}",
            check.id,
            check.owner
        );
        assert!(
            allowed_levels.contains(check.level.as_str()),
            "unknown level for {}: {}",
            check.id,
            check.level
        );
        assert!(
            check.required,
            "all catalog checks must be required: {}",
            check.id
        );
        assert!(
            !check.command.trim().is_empty(),
            "empty command: {}",
            check.id
        );
        assert!(!check.platforms.is_empty(), "no platforms: {}", check.id);
        assert!(!check.sources.is_empty(), "no source paths: {}", check.id);
        assert!(
            !check.evidence.is_empty(),
            "no evidence mapping: {}",
            check.id
        );

        for platform in &check.platforms {
            assert!(
                allowed_platforms.contains(platform.as_str()),
                "unknown platform for {}: {}",
                check.id,
                platform
            );
        }
        for source in &check.sources {
            assert_repository_path(&root, source);
        }
        assert!(
            documentation.contains(&format!("`{}`", check.id)),
            "verification README does not mention {}",
            check.id
        );
    }

    assert_eq!(manifest.check.len(), REQUIRED_IDS.len());
    for required in REQUIRED_IDS {
        assert!(
            ids.contains(required),
            "required check ID is missing: {required}"
        );
    }
}
