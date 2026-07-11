use std::io::Write;
use std::process::{Command, Stdio};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_ptymark")
}

#[test]
fn preview_renders_before_stdout_display() {
    let mut child = Command::new(binary())
        .arg("preview")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ptymark");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"before\n$$\nE = mc^2\n$$\nafter\n")
        .expect("write input");
    let output = child.wait_with_output().expect("wait");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    assert!(stdout.starts_with("before\n"));
    assert!(stdout.contains("ptymark math preview"));
    assert!(stdout.ends_with("after\n"));
    assert!(!stdout.contains("$$"));
}

#[test]
fn source_preview_is_lossless() {
    let source = b"before\n```mermaid\nA --> B\n```\nafter\n";
    let mut child = Command::new(binary())
        .args(["preview", "--source"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn ptymark");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(source)
        .expect("write input");
    let output = child.wait_with_output().expect("wait");

    assert!(output.status.success());
    assert_eq!(output.stdout, source);
}

#[test]
fn terminal_sequences_are_not_interpreted_as_markdown() {
    let source = b"\x1b[?1049h$$\nE = mc^2\n$$\n\x1b[?1049l";
    let mut child = Command::new(binary())
        .arg("preview")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn ptymark");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(source)
        .expect("write input");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success());
    assert_eq!(output.stdout, source);
}

#[test]
fn help_and_version_are_stable_public_entrypoints() {
    let help = Command::new(binary()).arg("--help").output().expect("help");
    assert!(help.status.success());
    let help_text = String::from_utf8(help.stdout).expect("UTF-8 help");
    assert!(help_text.contains("ptymark -- COMMAND"));
    assert!(help_text.contains("ptymark preview"));
    assert!(help_text.contains("--no-cache"));

    let version = Command::new(binary())
        .arg("--version")
        .output()
        .expect("version");
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout)
            .expect("UTF-8 version")
            .trim(),
        concat!("ptymark ", env!("CARGO_PKG_VERSION"))
    );
}

#[cfg(unix)]
#[test]
fn command_mode_preserves_child_exit_status() {
    let status = Command::new(binary())
        .args(["--", "/bin/sh", "-c", "exit 7"])
        .status()
        .expect("run command mode");
    assert_eq!(status.code(), Some(7));
}
