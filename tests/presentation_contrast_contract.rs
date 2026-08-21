#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const MATH_SOURCE: &[u8] = b"$$\nE = mc^2\n$$\n";

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

fn write_executable(path: &Path, source: &str) {
    fs::write(path, source).expect("write executable");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn shell_path(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\"'\"'"))
}

fn write_config(path: &Path, tex2svg: &Path, presenter: &Path, color: &str) {
    fs::write(
        path,
        format!(
            "schema_version = 2\ndefault_profile = \"default\"\n\n[profiles.default.presentation]\nmode = \"symbols\"\ncolor = \"{color}\"\nfallback_columns = 80\n\n[profiles.default.engines.math]\nprovider = \"external\"\nprogram = \"{}\"\n\n[profiles.default.engines.presenter]\nprovider = \"external\"\nprogram = \"{}\"\n",
            toml_path(tex2svg),
            toml_path(presenter),
        ),
    )
    .expect("write config");
}

fn preview(config: &Path, options: &[&str]) -> Output {
    let mut child = Command::new(binary())
        .arg("--config")
        .arg(config)
        .arg("preview")
        .args(options)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn preview");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(MATH_SOURCE)
        .expect("write input");
    child.wait_with_output().expect("wait for preview")
}

#[test]
fn color_policy_maps_to_the_presenter_without_tty_guessing() {
    let root = temp_root("presentation-color-policy");
    let tex2svg = root.join("tex2svg");
    let presenter = root.join("presenter");
    let arguments = root.join("presenter-arguments.txt");
    let config = root.join("ptymark.toml");

    write_executable(
        &tex2svg,
        "#!/bin/sh\nprintf '<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>\\n'\n",
    );
    write_executable(
        &presenter,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >{}\nprintf 'visible math\\n'\n",
            shell_path(&arguments),
        ),
    );

    for (policy, cli_color, expected) in [
        ("auto", false, "none"),
        ("auto", true, "full"),
        ("always", false, "full"),
        ("never", true, "none"),
    ] {
        write_config(&config, &tex2svg, &presenter, policy);
        let options = if cli_color {
            &["--color"][..]
        } else {
            &[][..]
        };
        let output = preview(&config, options);
        assert!(output.status.success(), "{policy}: {output:?}");
        assert_eq!(output.stdout, b"visible math\n");

        let values = fs::read_to_string(&arguments).expect("presenter arguments");
        let values: Vec<_> = values.lines().collect();
        let color_index = values
            .iter()
            .position(|value| *value == "--colors")
            .expect("--colors argument");
        assert_eq!(values.get(color_index + 1), Some(&expected), "{policy}");
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn presenter_failure_restores_exact_source_and_safe_modes_bypass_engines() {
    let root = temp_root("presentation-fallback");
    let tex2svg = root.join("tex2svg");
    let presenter = root.join("presenter");
    let started = root.join("renderer-started");
    let config = root.join("ptymark.toml");

    write_executable(
        &tex2svg,
        &format!(
            "#!/bin/sh\ntouch {}\nprintf '<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>\\n'\n",
            shell_path(&started),
        ),
    );
    write_executable(
        &presenter,
        "#!/bin/sh\nprintf 'zero-contrast presentation rejected\\n' >&2\nexit 9\n",
    );
    write_config(&config, &tex2svg, &presenter, "always");

    let fallback = preview(&config, &[]);
    assert!(fallback.status.success(), "{fallback:?}");
    assert_eq!(fallback.stdout, MATH_SOURCE);
    assert!(started.is_file());

    fs::remove_file(&started).expect("clear renderer marker");
    for mode in ["--source", "--safe"] {
        let output = preview(&config, &[mode]);
        assert!(output.status.success(), "{mode}: {output:?}");
        assert_eq!(output.stdout, MATH_SOURCE);
        assert!(!started.exists(), "{mode} must not start external engines");
    }

    let strict = preview(&config, &["--strict"]);
    assert!(!strict.status.success());

    let _ = fs::remove_dir_all(root);
}
