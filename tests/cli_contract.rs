use std::fs;
use std::io::Write;
use std::path::PathBuf;
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
    let path = std::env::temp_dir().join(format!("ptymark-{label}-{nonce}"));
    fs::create_dir_all(&path).expect("temp root");
    path
}

#[test]
fn preview_renders_before_stdout() {
    let mut child = Command::new(binary())
        .arg("preview")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"before\n$$\nE = mc^2\n$$\nafter\n")
        .expect("write");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(text.starts_with("before\n"));
    assert!(text.contains("ptymark math"));
    assert!(text.ends_with("after\n"));
}

#[test]
fn explicit_source_mode_is_lossless() {
    let source = b"```mermaid\nflowchart LR\n  A --> B\n```\n";
    let mut child = Command::new(binary())
        .args(["preview", "--source"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(source)
        .expect("write");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success());
    assert_eq!(output.stdout, source);
}

#[test]
fn config_check_rejects_unknown_keys() {
    let root = temp_root("config");
    let path = root.join("ptymark.toml");
    fs::write(&path, "schema_version = 1\nunknown = true\n").expect("write config");
    let output = Command::new(binary())
        .args(["config", "check", "--config"])
        .arg(path)
        .output()
        .expect("config check");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown field"));
}

#[cfg(unix)]
#[test]
fn command_mode_preserves_exit_status() {
    let status = Command::new(binary())
        .args(["--", "/bin/sh", "-c", "exit 7"])
        .status()
        .expect("command mode");
    assert_eq!(status.code(), Some(7));
}
