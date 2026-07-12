use ptymark::{ConfigEnvironment, ConfigManager, ConfigRequest, RuntimeBuilder, RuntimeRequest};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ptymark-extension-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("create temp root");
    root
}

fn write_config(root: &Path, content: &str) -> PathBuf {
    let path = root.join("config.toml");
    fs::write(&path, content).expect("write config");
    path
}

fn load(path: &Path, profile: &str) -> Result<ptymark::LoadedConfig, ptymark::ConfigError> {
    ConfigManager::default().load(
        ConfigRequest {
            explicit_path: Some(path.to_path_buf()),
            profile: Some(profile.to_owned()),
            no_config: false,
            working_directory: path.parent().expect("parent").to_path_buf(),
        },
        ConfigEnvironment::default(),
    )
}

fn custom_engine_config(name: &str, execution: &str, program: &str) -> String {
    format!(
        r#"
schema_version = 1

[profiles.custom]
extends = "interactive"

[profiles.custom.engines.math]
candidates = ["{name}", "source"]
preferred_artifacts = ["text/plain"]

[engines.{name}]
type = "process"
version = "1"
semantic_kinds = ["math"]
artifact_types = ["text/plain"]
layout = "independent"
execution = "{execution}"
program = "{program}"
args = []
timeout_ms = 1000
max_stdout_bytes = 4096
max_stderr_bytes = 4096
"#
    )
}

#[test]
fn external_engine_cannot_redefine_a_reserved_builtin_id() {
    let root = temp_root("reserved-id");
    let path = write_config(
        &root,
        &custom_engine_config("preview", "one-shot", "/bin/cat"),
    );
    let error = load(&path, "custom").expect_err("reserved ID must fail validation");
    assert!(error.to_string().contains("reserved built-in engine ID"));
}

#[test]
fn persistent_custom_engine_requires_a_dedicated_worker_provider() {
    let root = temp_root("persistent");
    let path = write_config(
        &root,
        &custom_engine_config("custom-math", "persistent-worker", "/bin/cat"),
    );
    let snapshot = load(&path, "custom")
        .expect("configuration is structurally valid")
        .into_snapshot(1)
        .expect("snapshot");
    let error = RuntimeBuilder::default()
        .build(snapshot, RuntimeRequest::preview())
        .err()
        .expect("persistent process requires a provider");
    assert!(error.to_string().contains("dedicated worker provider"));
}

#[test]
fn user_configured_process_engine_requires_an_absolute_program() {
    let root = temp_root("relative-program");
    let path = write_config(
        &root,
        &custom_engine_config("custom-math", "one-shot", "cat"),
    );
    let snapshot = load(&path, "custom")
        .expect("configuration is structurally valid")
        .into_snapshot(1)
        .expect("snapshot");
    let error = RuntimeBuilder::default()
        .build(snapshot, RuntimeRequest::preview())
        .err()
        .expect("relative program must fail before rendering");
    assert!(error.to_string().contains("absolute program path"));
}

#[test]
fn effective_config_redacts_values_but_fingerprint_material_distinguishes_them() {
    let root = temp_root("redaction");
    let first = write_config(
        &root,
        &(custom_engine_config("custom-math", "one-shot", "/bin/cat")
            + "\n[engines.custom-math.environment]\nTOKEN = \"first-secret\"\n"),
    );
    let loaded_first = load(&first, "custom").expect("first config");
    let shown = loaded_first.effective_toml().expect("effective TOML");
    assert!(shown.contains("<redacted>"));
    assert!(!shown.contains("first-secret"));
    let first_identity = loaded_first
        .fingerprint_material()
        .expect("first fingerprint material");

    fs::write(
        &first,
        custom_engine_config("custom-math", "one-shot", "/bin/cat")
            + "\n[engines.custom-math.environment]\nTOKEN = \"second-secret\"\n",
    )
    .expect("rewrite config");
    let second_identity = load(&first, "custom")
        .expect("second config")
        .fingerprint_material()
        .expect("second fingerprint material");
    assert_ne!(first_identity, second_identity);
}
