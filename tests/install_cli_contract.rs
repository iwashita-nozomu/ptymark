#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_ptymark")
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ptymark-install-cli-{label}-{nonce}"));
    fs::create_dir_all(&path).expect("temp root");
    path
}

fn executable(path: &Path) {
    fs::write(path, "#!/bin/sh\nexit 0\n").expect("write executable");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

#[test]
fn install_resolve_separates_user_intent_from_resolved_state() {
    let root = temp_root("resolve");
    let mmdc = root.join("mmdc");
    let chafa = root.join("chafa");
    let config = root.join("config/ptymark.toml");
    let state = root.join("state/install.toml");
    executable(&mmdc);
    executable(&chafa);

    let output = Command::new(binary())
        .args(["install", "resolve", "--config"])
        .arg(&config)
        .arg("--state")
        .arg(&state)
        .arg("--mermaid")
        .arg(&mmdc)
        .args(["--math", "preview", "--presenter"])
        .arg(&chafa)
        .output()
        .expect("install resolve");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config_text = fs::read_to_string(&config).expect("config");
    assert!(config_text.contains("schema_version = 2"));
    assert!(config_text.contains("provider = \"external\""));
    assert!(config_text.contains(&mmdc.display().to_string()));
    assert!(!config_text.contains("backend = \"mermaid-cli\""));

    let state_text = fs::read_to_string(&state).expect("state");
    assert!(state_text.contains(&mmdc.display().to_string()));
    assert!(state_text.contains(&chafa.display().to_string()));

    let status = Command::new(binary())
        .args(["install", "status", "--state"])
        .arg(&state)
        .output()
        .expect("install status");
    assert!(status.status.success());
    let stdout = String::from_utf8(status.stdout).expect("status UTF-8");
    assert!(stdout.contains("mermaid\tmermaid-cli\tready"));
    assert!(stdout.contains("presenter\tchafa-symbols\tready"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn install_dry_run_does_not_write_files() {
    let root = temp_root("dry-run");
    let config = root.join("config/ptymark.toml");
    let state = root.join("state/install.toml");
    let output = Command::new(binary())
        .args(["install", "resolve", "--config"])
        .arg(&config)
        .arg("--state")
        .arg(&state)
        .args(["--mermaid", "preview", "--math", "preview", "--dry-run"])
        .output()
        .expect("dry run");
    assert!(output.status.success());
    assert!(!config.exists());
    assert!(!state.exists());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("user-authored ptymark configuration"));
    assert!(stdout.contains("internal installation state"));
    let _ = fs::remove_dir_all(root);
}
