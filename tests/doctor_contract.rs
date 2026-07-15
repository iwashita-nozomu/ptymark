use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_ptymark")
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ptymark-doctor-contract-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("temp root");
    root
}

fn isolated_command(root: &Path) -> Command {
    let mut command = Command::new(binary());
    command
        .env("HOME", root.join("home"))
        .env("USERPROFILE", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("xdg-config"))
        .env("XDG_STATE_HOME", root.join("xdg-state"))
        .env("APPDATA", root.join("appdata"));
    command
}

fn doctor(root: &Path, arguments: &[&str]) -> Output {
    isolated_command(root)
        .args(arguments)
        .output()
        .expect("run doctor")
}

#[test]
fn doctor_json_has_the_v1_schema_and_ready_exit() {
    let root = temp_root("ready");
    let output = doctor(&root, &["doctor", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = String::from_utf8(output.stdout).expect("UTF-8 JSON");
    assert!(json.contains("\"schema\": \"ptymark.doctor.v1\""));
    assert!(json.contains("\"status\": \"ready\""));
    assert!(json.contains("\"install.state_missing\""));
    assert!(!json.contains(root.to_string_lossy().as_ref()));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_config_is_unusable_with_a_stable_finding() {
    let root = temp_root("invalid-config");
    let config = root.join("invalid.toml");
    fs::write(&config, "schema_version = 999\nsecret = 'token-123'\n").expect("write config");
    let output = doctor(
        &root,
        &[
            "doctor",
            "--json",
            "--config",
            config.to_str().expect("path"),
        ],
    );
    assert_eq!(output.status.code(), Some(20));
    let json = String::from_utf8(output.stdout).expect("UTF-8 JSON");
    assert!(json.contains("\"status\": \"unusable\""));
    assert!(json.contains("\"config.invalid\""));
    assert!(!json.contains("token-123"));
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
fn executable(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, body).expect("write executable");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

#[cfg(unix)]
#[test]
fn default_doctor_inspects_executables_without_starting_them() {
    let root = temp_root("side-effect-free");
    let marker = root.join("started");
    let renderer = root.join("mmdc");
    let presenter = root.join("chafa");
    executable(
        &renderer,
        &format!("#!/bin/sh\ntouch '{}'\nexit 99\n", marker.display()),
    );
    executable(
        &presenter,
        &format!("#!/bin/sh\ntouch '{}'\nexit 99\n", marker.display()),
    );
    let config = root.join("config.toml");
    fs::write(
        &config,
        format!(
            "schema_version = 1\n\n[engines.mermaid]\nbackend = 'mermaid-cli'\npath = '{}'\n\n[engines.presenter]\npath = '{}'\n",
            renderer.display(),
            presenter.display()
        ),
    )
    .expect("write config");

    let output = doctor(
        &root,
        &[
            "doctor",
            "--json",
            "--config",
            config.to_str().expect("path"),
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists(), "doctor started an external process");
    let json = String::from_utf8(output.stdout).expect("UTF-8 JSON");
    assert!(json.contains("\"status\": \"ready\""));
    assert!(json.contains("\"state\": \"ready\""));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_external_engine_is_degraded_or_unusable_when_strict() {
    let root = temp_root("missing-engine");
    let config = root.join("config.toml");
    fs::write(
        &config,
        "schema_version = 1\n\n[engines.math]\nbackend = 'mathjax-cli'\npath = 'definitely-missing-ptymark-engine'\n",
    )
    .expect("write config");
    let path = config.to_str().expect("path");

    let degraded = doctor(&root, &["doctor", "--json", "--config", path]);
    assert_eq!(degraded.status.code(), Some(10));
    assert!(String::from_utf8_lossy(&degraded.stdout).contains("engine.missing"));

    let unusable = doctor(&root, &["doctor", "--json", "--strict", "--config", path]);
    assert_eq!(unusable.status.code(), Some(20));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn support_report_is_redacted_and_refuses_overwrite() {
    let root = temp_root("support-report");
    let report = root.join("support.json");
    let path = report.to_str().expect("path");
    let first = doctor(&root, &["doctor", "--support-report", path, "--private"]);
    assert_eq!(first.status.code(), Some(0));
    let source = fs::read_to_string(&report).expect("read report");
    assert!(source.contains("ptymark.doctor.v1"));
    assert!(source.contains("mode.private"));
    assert!(!source.contains(root.to_string_lossy().as_ref()));

    let second = doctor(&root, &["doctor", "--support-report", path]);
    assert_eq!(second.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&second.stderr).contains("already exists"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn conflicting_modes_fail_before_doctor_collection() {
    let root = temp_root("conflict");
    let output = doctor(&root, &["doctor", "--source", "--safe"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("`--source` and `--safe` cannot be combined")
    );
    let _ = fs::remove_dir_all(root);
}
