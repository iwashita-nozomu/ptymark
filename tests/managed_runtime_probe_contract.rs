#![cfg(unix)]

use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_ptymark")
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ptymark-managed-probe-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("temp root");
    root
}

fn executable(path: &Path, body: &str) {
    fs::write(path, body).expect("write executable");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

fn managed_bundle(root: &Path, body: &str) -> (PathBuf, PathBuf, PathBuf) {
    let bin = root.join("bundle/bin");
    let app = root.join("bundle/app");
    fs::create_dir_all(&bin).expect("bin");
    fs::create_dir_all(&app).expect("app");
    let node = root.join("bundle/node");
    let browser = root.join("bundle/chrome");
    fs::write(&node, b"node").expect("node");
    fs::write(&browser, b"browser").expect("browser");

    let mermaid = bin.join("mmdc");
    let math = bin.join("tex2svg");
    let presenter = bin.join("chafa");
    for alias in [&mermaid, &math, &presenter] {
        executable(alias, body);
    }
    fs::write(
        root.join("bundle/bundle.toml"),
        format!(
            "schema_version = 1\nnode_path = {:?}\napp_root = {:?}\ncache_root = {:?}\nbrowser_path = {:?}\n",
            node,
            app,
            root.join("bundle/cache"),
            browser,
        ),
    )
    .expect("manifest");
    (mermaid, math, presenter)
}

fn config(root: &Path, mermaid: &Path, math: &Path, presenter: &Path) -> PathBuf {
    let path = root.join("config.toml");
    fs::write(
        &path,
        format!(
            "schema_version = 1\n\n[engines.mermaid]\nbackend = 'mermaid-cli'\npath = '{}'\n\n[engines.math]\nbackend = 'mathjax-cli'\npath = '{}'\n\n[engines.presenter]\npath = '{}'\n",
            mermaid.display(),
            math.display(),
            presenter.display(),
        ),
    )
    .expect("config");
    path
}

fn isolated_command(root: &Path) -> Command {
    let mut command = Command::new(binary());
    command
        .env("HOME", root.join("home"))
        .env("USERPROFILE", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("xdg-config"))
        .env("XDG_STATE_HOME", root.join("xdg-state"))
        .env("APPDATA", root.join("appdata"))
        .env("LOCALAPPDATA", root.join("local-appdata"));
    command
}

fn doctor(root: &Path, config: &Path) -> Output {
    isolated_command(root)
        .env("TERM", "tmux-256color")
        .env("TMUX", "/tmp/private-tmux/socket,1,0")
        .args(["doctor", "--json", "--config"])
        .arg(config)
        .output()
        .expect("doctor")
}

#[test]
fn doctor_separates_managed_files_from_bounded_runtime_readback() {
    let root = temp_root("ready");
    let body = r#"#!/bin/sh
[ "${TERM:-}" = dumb ] || exit 41
[ -z "${TMUX:-}" ] || exit 42
touch "$(dirname "$0")/../$(basename "$0").started"
case "$(basename "$0")" in
  mmdc)
    output=
    while [ "$#" -gt 0 ]; do
      if [ "$1" = --output ]; then output=$2; shift 2; else shift; fi
    done
    printf '<svg xmlns="http://www.w3.org/2000/svg"></svg>\n' > "$output"
    ;;
  tex2svg)
    printf '<svg xmlns="http://www.w3.org/2000/svg"></svg>\n'
    ;;
  chafa)
    printf '#\n'
    ;;
esac
"#;
    let (mermaid, math, presenter) = managed_bundle(&root, body);
    let config = config(&root, &mermaid, &math, &presenter);

    let output = doctor(&root, &config);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("doctor JSON");
    assert_eq!(json["engines"][0]["state"], "ready");
    assert_eq!(json["engines"][0]["runtime_state"], "ready");
    assert_eq!(json["engines"][0]["browser_state"], "present");
    assert_eq!(json["engines"][1]["runtime_state"], "ready");
    assert_eq!(json["presenter"]["state"], "ready");
    assert_eq!(json["presenter"]["runtime_state"], "ready");
    for role in ["mmdc", "tex2svg", "chafa"] {
        assert!(root.join(format!("bundle/{role}.started")).is_file());
    }
    let public = String::from_utf8(output.stdout).expect("UTF-8 JSON");
    assert!(!public.contains(root.to_string_lossy().as_ref()));
    assert!(!public.contains("private-tmux"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_linux_libraries_are_stable_redacted_and_actionable() {
    let root = temp_root("missing-libraries");
    let body = r#"#!/bin/sh
printf '/home/alice/private/chrome: error while loading shared libraries: libnspr4.so: cannot open shared object file: No such file or directory\n' >&2
printf 'libnss3.so => not found\nlibnssutil3.so => not found\nlibsmime3.so => not found\n' >&2
exit 127
"#;
    let (mermaid, math, presenter) = managed_bundle(&root, body);
    let config = config(&root, &mermaid, &math, &presenter);

    let output = doctor(&root, &config);
    assert_eq!(output.status.code(), Some(10));
    let public = String::from_utf8(output.stdout).expect("UTF-8 JSON");
    assert!(public.contains("browser.runtime_libraries_missing"));
    assert!(public.contains("libnspr4.so"));
    assert!(public.contains("libnss3.so"));
    assert!(public.contains("libnssutil3.so"));
    assert!(public.contains("libsmime3.so"));
    assert!(public.contains("apt-get install --yes libnspr4 libnss3"));
    assert!(public.contains("\"state\": \"ready\""));
    assert!(public.contains("\"runtime_state\": \"missing-libraries\""));
    assert!(!public.contains("/home/alice"));
    assert!(!public.contains("cannot open shared object file"));
    assert!(!public.contains(root.to_string_lossy().as_ref()));

    let state = root.join("state.toml");
    fs::write(
        &state,
        format!(
            "schema_version = 1\nptymark_version = {:?}\nconfig_path = {:?}\n\n[[components]]\nrole = 'mermaid'\nbackend = 'mermaid-cli'\nactive = true\norigin = 'managed'\nresolved_path = {:?}\n\n[[components]]\nrole = 'math'\nbackend = 'mathjax-cli'\nactive = true\norigin = 'managed'\nresolved_path = {:?}\n\n[[components]]\nrole = 'presenter'\nbackend = 'chafa-symbols'\nactive = true\norigin = 'managed'\nresolved_path = {:?}\n",
            env!("CARGO_PKG_VERSION"),
            config,
            mermaid,
            math,
            presenter,
        ),
    )
    .expect("state");
    let status = isolated_command(&root)
        .args(["install", "status", "--state"])
        .arg(&state)
        .output()
        .expect("install status");
    assert!(status.status.success());
    let stdout = String::from_utf8(status.stdout).expect("status UTF-8");
    assert!(stdout.contains("mermaid\tmermaid-cli\tmissing"));
    assert!(stdout.contains("math\tmathjax-cli\tmissing"));
    assert!(stdout.contains("presenter\tchafa-symbols\tmissing"));
    assert!(!stdout.contains("\tready\t"));
    let _ = fs::remove_dir_all(root);
}
