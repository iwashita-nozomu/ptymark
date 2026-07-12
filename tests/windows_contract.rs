#![cfg(windows)]

use ptymark::resolve_executable;
use std::fs;
use std::path::PathBuf;
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
fn absolute_program_resolution_honors_native_exe_suffix() {
    let root = root("pathext");
    let executable = root.join("renderer.exe");
    fs::write(&executable, b"native-test-placeholder").expect("placeholder");
    let resolved = resolve_executable(&root.join("renderer")).expect("resolve .exe");
    assert_eq!(resolved, fs::canonicalize(&executable).expect("canonical"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn batch_renderer_is_rejected_without_a_shell_handoff() {
    let root = root("batch");
    let script = root.join("renderer.cmd");
    fs::write(&script, "@echo off\r\necho unsafe-shell-wrapper\r\n").expect("script");
    let error = resolve_executable(&script).expect_err("batch wrapper must be rejected");
    assert!(error.to_string().contains("shell wrapper"));
    let _ = fs::remove_dir_all(root);
}
