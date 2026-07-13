use std::ffi::OsString;
use std::process::{Command, Output};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_ptymark")
}

fn run_interactive(child_command: Vec<OsString>) -> Output {
    Command::new(binary())
        .args(["--config", "examples/ptymark.toml", "--"])
        .args(child_command)
        .output()
        .expect("run child in native PTY")
}

#[cfg(unix)]
fn markdown_command() -> Vec<OsString> {
    vec![
        OsString::from("/bin/sh"),
        OsString::from("-c"),
        OsString::from("printf 'before\\n$$\\nE = mc^2\\n$$\\nafter\\n'"),
    ]
}

#[cfg(windows)]
fn markdown_command() -> Vec<OsString> {
    vec![
        OsString::from("powershell.exe"),
        OsString::from("-NoLogo"),
        OsString::from("-NoProfile"),
        OsString::from("-NonInteractive"),
        OsString::from("-Command"),
        OsString::from(
            "[Console]::Out.Write((@('before','$$','E = mc^2','$$','after') -join \"`n\") + \"`n\")",
        ),
    ]
}

#[cfg(unix)]
fn tty_probe_command() -> Vec<OsString> {
    vec![
        OsString::from("/bin/sh"),
        OsString::from("-c"),
        OsString::from("test -t 0 && test -t 1 && printf TTY_OK"),
    ]
}

#[cfg(windows)]
fn tty_probe_command() -> Vec<OsString> {
    vec![
        OsString::from("powershell.exe"),
        OsString::from("-NoLogo"),
        OsString::from("-NoProfile"),
        OsString::from("-NonInteractive"),
        OsString::from("-Command"),
        OsString::from(
            "if (-not [Console]::IsInputRedirected -and -not [Console]::IsOutputRedirected) { [Console]::Out.Write('TTY_OK'); exit 0 } else { [Console]::Error.Write(\"stdin=$([Console]::IsInputRedirected) stdout=$([Console]::IsOutputRedirected)\"); exit 9 }",
        ),
    ]
}

#[cfg(unix)]
fn exit_command(code: u8) -> Vec<OsString> {
    vec![
        OsString::from("/bin/sh"),
        OsString::from("-c"),
        OsString::from(format!("exit {code}")),
    ]
}

#[cfg(windows)]
fn exit_command(code: u8) -> Vec<OsString> {
    vec![
        OsString::from("powershell.exe"),
        OsString::from("-NoLogo"),
        OsString::from("-NoProfile"),
        OsString::from("-NonInteractive"),
        OsString::from("-Command"),
        OsString::from(format!("exit {code}")),
    ]
}

#[cfg(unix)]
fn alternate_screen_command() -> Vec<OsString> {
    vec![
        OsString::from("/bin/sh"),
        OsString::from("-c"),
        OsString::from("printf '\\033[?1049h$$\\nE = mc^2\\n$$\\n\\033[?1049l'"),
    ]
}

#[cfg(windows)]
fn alternate_screen_command() -> Vec<OsString> {
    vec![
        OsString::from("powershell.exe"),
        OsString::from("-NoLogo"),
        OsString::from("-NoProfile"),
        OsString::from("-NonInteractive"),
        OsString::from("-Command"),
        OsString::from(
            "$e=[char]27; [Console]::Out.Write($e + '[?1049h' + '$$' + \"`nE = mc^2`n\" + '$$' + \"`n\" + $e + '[?1049l')",
        ),
    ]
}

#[test]
fn native_pty_or_conpty_renders_real_child_output() {
    let output = run_interactive(markdown_command());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("before"), "{text}");
    assert!(text.contains("ptymark math"), "{text}");
    assert!(text.contains("after"), "{text}");
    assert!(!text.contains("$$"), "{text}");
}

#[test]
fn child_observes_a_real_terminal() {
    let output = run_interactive(tty_probe_command());
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("TTY_OK"));
}

#[test]
fn child_exit_status_is_preserved() {
    let output = run_interactive(exit_command(7));
    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn alternate_screen_output_is_not_semantically_rendered() {
    let output = run_interactive(alternate_screen_command());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = output.stdout;
    assert!(
        bytes
            .windows(b"\x1b[?1049h".len())
            .any(|window| window == b"\x1b[?1049h")
    );
    assert!(bytes.windows(2).any(|window| window == b"$$"));
    assert!(
        bytes
            .windows(b"\x1b[?1049l".len())
            .any(|window| window == b"\x1b[?1049l")
    );
}

#[cfg(unix)]
#[test]
fn ctrl_c_byte_reaches_the_foreground_process_group() {
    use std::io::Write;
    use std::process::Stdio;
    use std::thread;
    use std::time::{Duration, Instant};

    let script = "trap 'printf \\\"INT_OK\\\\n\\\"; exit 130' INT; while :; do sleep 1; done";
    let mut child = Command::new(binary())
        .args([
            "--config",
            "examples/ptymark.toml",
            "--",
            "/bin/sh",
            "-c",
            script,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn interactive PTY wrapper");

    thread::sleep(Duration::from_millis(300));
    child
        .stdin
        .take()
        .expect("outer stdin")
        .write_all(&[0x03])
        .expect("send Ctrl-C byte");

    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if child.try_wait().expect("poll child").is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            panic!("Ctrl-C did not terminate the trapped foreground process");
        }
        thread::sleep(Duration::from_millis(50));
    }

    let output = child.wait_with_output().expect("collect Ctrl-C output");
    assert_eq!(output.status.code(), Some(130));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("INT_OK"),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
