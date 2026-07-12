use std::io::Write;
use std::process::{Command, Stdio};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_ptymark")
}

#[test]
fn filtered_run_renders_child_stdout() {
    let mut child = Command::new(binary())
        .args([
            "--config",
            "examples/ptymark.toml",
            "run",
            "--",
            binary(),
            "preview",
            "--source",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn filtered run");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"before\n$$\nE = mc^2\n$$\nafter\n")
        .expect("write");

    let output = child.wait_with_output().expect("wait");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(text.starts_with("before\n"));
    assert!(text.contains("ptymark math"));
    assert!(text.ends_with("after\n"));
    assert!(!text.contains("$$"));
}

#[test]
fn filtered_run_preserves_child_exit_status() {
    let status = Command::new(binary())
        .args([
            "--config",
            "examples/ptymark.toml",
            "run",
            "--",
            binary(),
            "--not-a-real-option",
        ])
        .status()
        .expect("filtered command mode");
    assert_eq!(status.code(), Some(2));
}

#[test]
fn top_level_and_run_help_describe_the_filtered_path() {
    let top_level = Command::new(binary())
        .arg("--help")
        .output()
        .expect("top-level help");
    assert!(top_level.status.success());
    assert!(
        String::from_utf8_lossy(&top_level.stdout).contains("run [OPTIONS] -- COMMAND")
    );

    let run_help = Command::new(binary())
        .args(["run", "--help"])
        .output()
        .expect("run help");
    assert!(run_help.status.success());
    assert!(
        String::from_utf8_lossy(&run_help.stdout).contains("stdout is a pipe, not a PTY")
    );
}
