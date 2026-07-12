#![cfg(windows)]

use ptymark::resolve_executable;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ptymark-windows-{label}-{nonce}"));
    fs::create_dir_all(&path).expect("temp root");
    path
}

#[test]
fn absolute_program_resolution_honors_pathext() {
    let root = root("pathext");
    let script = root.join("renderer.cmd");
    fs::write(&script, "@echo off\r\necho ok\r\n").expect("script");
    let resolved = resolve_executable(&root.join("renderer")).expect("resolve .cmd");
    assert_eq!(resolved, fs::canonicalize(&script).expect("canonical"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn fixed_managed_batch_wrapper_can_be_invoked() {
    let root = root("batch");
    let script = root.join("presenter.cmd");
    fs::write(&script, "@echo off\r\necho windows-managed-output\r\n").expect("script");
    let output = Command::new(&script).output().expect("run .cmd");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("windows-managed-output"));
    let _ = fs::remove_dir_all(root);
}
