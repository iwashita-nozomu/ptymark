use ptymark::{
    Config, EnginePreference, InstallError, InstallRequest, InstallState, Installer, MathEngine,
    MermaidEngine, PresenterPreference, ProgramResolver,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Default)]
struct FakeResolver {
    programs: HashMap<PathBuf, PathBuf>,
}

impl FakeResolver {
    fn with(mut self, configured: impl Into<PathBuf>, resolved: impl Into<PathBuf>) -> Self {
        self.programs.insert(configured.into(), resolved.into());
        self
    }
}

impl ProgramResolver for FakeResolver {
    fn resolve(&self, configured: &Path) -> Result<PathBuf, InstallError> {
        self.programs
            .get(configured)
            .cloned()
            .ok_or_else(|| InstallError::new(format!("{} is unavailable", configured.display())))
    }
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ptymark-install-{label}-{nonce}"));
    fs::create_dir_all(&path).expect("temp root");
    path
}

#[test]
fn first_install_resolves_available_engines_and_records_absolute_paths() {
    let root = temp_root("initial");
    let config_path = root.join("config/ptymark.toml");
    let state_path = root.join("state/install.toml");
    let resolver = FakeResolver::default()
        .with("mmdc", "/resolved/bin/mmdc")
        .with("chafa", "/resolved/bin/chafa");
    let installer = Installer::new(resolver);
    let request = InstallRequest::new(config_path.clone(), state_path.clone());

    let plan = installer.plan(&request).expect("plan");
    assert_eq!(plan.config.engines.mermaid.backend, MermaidEngine::MermaidCli);
    assert_eq!(
        plan.config.engines.mermaid.path,
        PathBuf::from("/resolved/bin/mmdc")
    );
    assert_eq!(plan.config.engines.math.backend, MathEngine::Preview);
    assert_eq!(
        plan.config.engines.presenter.path,
        PathBuf::from("/resolved/bin/chafa")
    );

    plan.apply().expect("apply");
    assert_eq!(Config::load_exact(&config_path).expect("config"), plan.config);
    let state = InstallState::load(&state_path).expect("state");
    assert_eq!(state.components.len(), 3);
    assert_eq!(state.config_path, config_path);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn automatic_external_selection_falls_back_when_presenter_is_missing() {
    let root = temp_root("fallback");
    let resolver = FakeResolver::default().with("mmdc", "/resolved/bin/mmdc");
    let installer = Installer::new(resolver);
    let request = InstallRequest::new(root.join("config.toml"), root.join("state.toml"));

    let plan = installer.plan(&request).expect("plan");
    assert_eq!(plan.config.engines.mermaid.backend, MermaidEngine::Preview);
    assert!(
        plan.warnings
            .iter()
            .any(|warning| warning.contains("presenter"))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn explicitly_requested_missing_engine_is_an_error() {
    let root = temp_root("missing");
    let installer = Installer::new(FakeResolver::default());
    let mut request = InstallRequest::new(root.join("config.toml"), root.join("state.toml"));
    request.mermaid = EnginePreference::External(PathBuf::from("/missing/mmdc"));
    request.presenter = PresenterPreference::Program(PathBuf::from("/missing/chafa"));

    let error = installer.plan(&request).expect_err("missing explicit engine");
    assert!(error.to_string().contains("cannot select mermaid"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rerun_replaces_one_engine_without_resetting_other_user_settings() {
    let root = temp_root("replace");
    let config_path = root.join("config.toml");
    let state_path = root.join("state.toml");
    let mut existing = Config::default();
    existing.detection.math = false;
    existing.engines.mermaid.backend = MermaidEngine::MermaidCli;
    existing.engines.mermaid.path = PathBuf::from("/old/mmdc");
    existing.engines.math.backend = MathEngine::Source;
    existing.engines.presenter.path = PathBuf::from("/old/chafa");
    fs::write(&config_path, existing.to_toml().expect("serialize")).expect("write config");

    let resolver = FakeResolver::default()
        .with("/new/mmdc", "/new/mmdc")
        .with("/old/chafa", "/old/chafa");
    let installer = Installer::new(resolver);
    let mut request = InstallRequest::new(config_path.clone(), state_path);
    request.mermaid = EnginePreference::External(PathBuf::from("/new/mmdc"));

    let plan = installer.plan(&request).expect("replace plan");
    assert!(!plan.config.detection.math);
    assert_eq!(plan.config.engines.math.backend, MathEngine::Source);
    assert_eq!(plan.config.engines.mermaid.backend, MermaidEngine::MermaidCli);
    assert_eq!(plan.config.engines.mermaid.path, PathBuf::from("/new/mmdc"));
    assert_eq!(
        plan.config.engines.presenter.path,
        PathBuf::from("/old/chafa")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reset_discards_existing_engine_choices_but_keeps_resolution_extensible() {
    let root = temp_root("reset");
    let config_path = root.join("config.toml");
    let mut existing = Config::default();
    existing.engines.math.backend = MathEngine::Source;
    fs::write(&config_path, existing.to_toml().expect("serialize")).expect("write config");

    let resolver = FakeResolver::default();
    let installer = Installer::new(resolver);
    let mut request = InstallRequest::new(config_path, root.join("state.toml"));
    request.reset = true;
    request.mermaid = EnginePreference::Preview;
    request.math = EnginePreference::Preview;

    let plan = installer.plan(&request).expect("reset plan");
    assert_eq!(plan.config.engines.math.backend, MathEngine::Preview);
    let _ = fs::remove_dir_all(root);
}
