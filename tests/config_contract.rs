use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_ptymark")
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ptymark-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("create temp root");
    root
}

fn write_config(root: &Path, content: &str) -> PathBuf {
    let path = root.join("config.toml");
    fs::write(&path, content).expect("write config");
    path
}

#[test]
fn config_check_and_show_resolve_a_named_profile() {
    let root = temp_root("config-show");
    let path = write_config(
        &root,
        r#"
schema_version = 1
default_profile = "compact"

[profiles.compact]
extends = "interactive"

[profiles.compact.detection]
max_buffer_bytes = 4096
max_line_bytes = 512

[profiles.compact.cache]
max_entries = 4
max_bytes = 65536
"#,
    );

    let check = Command::new(binary())
        .args(["config", "check", "--config"])
        .arg(&path)
        .env("PTYMARK_NO_CONFIG", "1")
        .output()
        .expect("config check");
    assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));
    assert!(String::from_utf8_lossy(&check.stdout).contains("profile=compact"));

    let show = Command::new(binary())
        .args(["config", "show", "--config"])
        .arg(&path)
        .args(["--profile", "compact"])
        .env("PTYMARK_NO_CONFIG", "1")
        .output()
        .expect("config show");
    assert!(show.status.success(), "{}", String::from_utf8_lossy(&show.stderr));
    let stdout = String::from_utf8(show.stdout).expect("UTF-8 config");
    assert!(stdout.contains("profile = \"compact\""));
    assert!(stdout.contains("max_buffer_bytes = 4096"));
    assert!(stdout.contains("max_entries = 4"));
}

#[test]
fn source_profile_keeps_semantic_source_lossless() {
    let root = temp_root("source-profile");
    let path = write_config(
        &root,
        r#"
schema_version = 1
default_profile = "copyable"

[profiles.copyable]
extends = "source"
"#,
    );
    let source = b"before\n```mermaid\nA --> B\n```\nafter\n";
    let mut child = Command::new(binary())
        .args(["preview", "--config"])
        .arg(&path)
        .env("PTYMARK_NO_CONFIG", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn preview");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(source)
        .expect("write source");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, source);
}

#[test]
fn detection_off_is_stricter_and_never_transforms_output() {
    let root = temp_root("detection-off");
    let path = write_config(
        &root,
        r#"
schema_version = 1

[profiles.off]
extends = "interactive"

[profiles.off.detection]
mode = "off"
"#,
    );
    let source = b"$$\nE = mc^2\n$$\n";
    let mut child = Command::new(binary())
        .args(["preview", "--config"])
        .arg(&path)
        .args(["--profile", "off"])
        .env("PTYMARK_NO_CONFIG", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn preview");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(source)
        .expect("write source");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, source);
}

#[test]
fn project_candidate_is_reported_but_not_implicitly_trusted() {
    let root = temp_root("paths");
    let output = Command::new(binary())
        .current_dir(&root)
        .args(["config", "paths", "--no-config"])
        .env_remove("PTYMARK_CONFIG")
        .env_remove("PTYMARK_PROFILE")
        .output()
        .expect("config paths");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 paths");
    assert!(stdout.contains("project\tuntrusted-project-not-loaded"));
    assert!(stdout.contains(".ptymark.toml"));
}

#[cfg(unix)]
#[test]
fn invalid_configuration_prevents_child_launch() {
    let root = temp_root("prelaunch-error");
    let path = write_config(
        &root,
        "schema_version = 1\nthis_is_a_typo = true\n",
    );
    let marker = root.join("child-started");
    let status = Command::new(binary())
        .args(["--", "/bin/sh", "-c"])
        .arg(format!("touch '{}'", marker.display()))
        .env("PTYMARK_CONFIG", &path)
        .env("PTYMARK_NO_CONFIG", "0")
        .status()
        .expect("run command mode");
    assert_eq!(status.code(), Some(2));
    assert!(!marker.exists(), "child must not start after config failure");
}
