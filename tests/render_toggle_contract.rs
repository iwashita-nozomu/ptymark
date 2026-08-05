use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const RENDER_TOGGLE_SEQUENCE: &[u8] = b"\xf4\x8f\xbf\xbd";

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_ptymark")
}

#[cfg(unix)]
fn render_toggle_command() -> Vec<OsString> {
    vec![
        OsString::from("/bin/sh"),
        OsString::from("-c"),
        OsString::from(
            "IFS= read -r _; printf 'first\\n$$\\nA = 1\\n$$\\n'; IFS= read -r _; printf 'second\\n$$\\nB = 2\\n$$\\n'",
        ),
    ]
}

#[cfg(windows)]
fn render_toggle_command() -> Vec<OsString> {
    vec![
        OsString::from("powershell.exe"),
        OsString::from("-NoLogo"),
        OsString::from("-NoProfile"),
        OsString::from("-NonInteractive"),
        OsString::from("-Command"),
        OsString::from(
            "$null=[Console]::In.ReadLine(); [Console]::Out.Write((@('first','$$','A = 1','$$') -join \"`n\") + \"`n\"); $null=[Console]::In.ReadLine(); [Console]::Out.Write((@('second','$$','B = 2','$$') -join \"`n\") + \"`n\")",
        ),
    ]
}

#[test]
fn toggle_pauses_future_rendering_in_a_real_pty_or_conpty_session() {
    let mut command = Command::new(binary());
    command
        .args(["--config", "examples/ptymark.toml", "--"])
        .args(render_toggle_command())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn interactive PTY wrapper");
    let mut input = child.stdin.take().expect("outer stdin");
    let mut output = child.stdout.take().expect("outer stdout");
    let mut errors = child.stderr.take().expect("outer stderr");

    let (output_sender, output_receiver) = mpsc::channel();
    let output_reader = thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            match output.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    if output_sender.send(buffer[..count].to_vec()).is_err() {
                        break;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => panic!("read wrapper stdout: {error}"),
            }
        }
    });
    let error_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        errors.read_to_end(&mut bytes).expect("read wrapper stderr");
        bytes
    });

    input.write_all(b"first\r").expect("release first block");
    input.flush().expect("flush first line");

    let first_deadline = Instant::now() + Duration::from_secs(10);
    let mut stdout = Vec::new();
    while !stdout
        .windows(b"ptymark math".len())
        .any(|window| window == b"ptymark math")
    {
        if Instant::now() >= first_deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "first block did not render: stdout={}",
                String::from_utf8_lossy(&stdout)
            );
        }
        if let Ok(bytes) = output_receiver.recv_timeout(Duration::from_millis(100)) {
            stdout.extend_from_slice(&bytes);
        }
    }

    input
        .write_all(RENDER_TOGGLE_SEQUENCE)
        .expect("send rendering toggle");
    input.write_all(b"second\r").expect("release second block");
    input.flush().expect("flush toggle and second line");
    drop(input);

    let exit_deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll toggled session") {
            break status;
        }
        if Instant::now() >= exit_deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("toggled session did not exit");
        }
        thread::sleep(Duration::from_millis(50));
    };

    output_reader.join().expect("stdout reader");
    while let Ok(bytes) = output_receiver.try_recv() {
        stdout.extend_from_slice(&bytes);
    }
    let stderr = error_reader.join().expect("stderr reader");

    assert!(
        status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );
    let text = String::from_utf8_lossy(&stdout);
    assert_eq!(text.matches("ptymark math").count(), 1, "{text}");
    assert!(text.contains("A = 1"), "{text}");
    assert!(text.contains("B = 2"), "{text}");
    assert_eq!(text.matches("$$").count(), 2, "{text}");
    assert!(
        !stdout
            .windows(RENDER_TOGGLE_SEQUENCE.len())
            .any(|window| window == RENDER_TOGGLE_SEQUENCE),
        "the reserved toggle value reached the child"
    );
}
