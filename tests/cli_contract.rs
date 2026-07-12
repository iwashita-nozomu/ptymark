use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::path::Path;
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

#[test]
fn engine_check_reports_dependency_free_defaults() {
    let output = Command::new(binary())
        .args(["engine", "check"])
        .output()
        .expect("engine check");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8 engine check");
    assert!(text.contains("mermaid\tpreview\tbuilt-in"));
    assert!(text.contains("math\tpreview\tbuilt-in"));
}

#[test]
fn help_lists_engine_check() {
    let output = Command::new(binary()).arg("--help").output().expect("help");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("engine check"));
}

#[cfg(unix)]
fn write_executable(path: &Path, source: &str) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(path, source).expect("write executable");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

#[cfg(unix)]
fn toml_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[cfg(unix)]
#[test]
fn configured_engine_paths_are_checked_and_used() {
    let root = temp_root("external-engine");
    let mmdc = root.join("mmdc");
    let chafa = root.join("chafa");
    let config_path = root.join("ptymark.toml");

    write_executable(
        &mmdc,
        "#!/bin/sh\nout=''\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    --output) out=$2; shift 2 ;;\n    *) shift ;;\n  esac\ndone\ncat >/dev/null\nprintf '<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>' >\"$out\"\n",
    );
    write_executable(&chafa, "#!/bin/sh\nprintf 'configured engine output\\n'\n");
    fs::write(
        &config_path,
        format!(
            "schema_version = 1\n\n[engines.mermaid]\nbackend = \"mermaid-cli\"\npath = \"{}\"\n\n[engines.math]\nbackend = \"preview\"\npath = \"tex2svg\"\n\n[engines.presenter]\npath = \"{}\"\n",
            toml_path(&mmdc),
            toml_path(&chafa)
        ),
    )
    .expect("write config");

    let check = Command::new(binary())
        .arg("--config")
        .arg(&config_path)
        .args(["engine", "check"])
        .output()
        .expect("engine check");
    assert!(check.status.success());
    let check_text = String::from_utf8(check.stdout).expect("UTF-8 check");
    assert!(check_text.contains("mermaid\tmermaid-cli"));
    assert!(check_text.contains("presenter\tchafa-symbols"));
    assert!(check_text.contains(&toml_path(&mmdc)));
    assert!(check_text.contains(&toml_path(&chafa)));

    let mut child = Command::new(binary())
        .arg("--config")
        .arg(&config_path)
        .arg("preview")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn preview");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"before\n```mermaid\nA --> B\n```\nafter\n")
        .expect("write");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"before\nconfigured engine output\nafter\n");

    let _ = fs::remove_dir_all(root);
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
