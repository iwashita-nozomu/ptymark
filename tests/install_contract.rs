use ptymark::{
    EnginePreference, EngineProvider, EngineSelection, InstallError, InstallRequest, InstallState,
    Installer, MathEngine, MermaidEngine, PresenterPreference, PresenterProvider,
    PresenterSelection, ProgramResolver, UserConfig,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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

fn program_path(root: &Path, group: &str, name: &str) -> PathBuf {
    let executable = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    root.join(group).join("bin").join(executable)
}

#[test]
fn first_install_separates_portable_intent_from_resolved_paths() {
    let root = tempfile::tempdir().expect("temp root");
    let config_path = root.path().join("config/ptymark.toml");
    let state_path = root.path().join("state/install.toml");
    let mmdc = program_path(root.path(), "resolved", "mmdc");
    let chafa = program_path(root.path(), "resolved", "chafa");
    let resolver = FakeResolver::default()
        .with("mmdc", &mmdc)
        .with("chafa", &chafa);
    let installer = Installer::new(resolver);
    let request = InstallRequest::new(config_path.clone(), state_path.clone());

    let plan = installer.plan(&request).expect("plan");
    assert_eq!(
        plan.config.engines.mermaid.backend,
        MermaidEngine::MermaidCli
    );
    assert_eq!(plan.config.engines.mermaid.path, mmdc);
    assert_eq!(plan.config.engines.math.backend, MathEngine::Preview);
    assert_eq!(plan.config.engines.presenter.path, chafa);

    let user_toml = plan.user_config.to_toml().expect("user TOML");
    assert!(user_toml.contains("provider = \"auto\""));
    assert!(!user_toml.contains("resolved"));
    assert!(!user_toml.contains("max_block_bytes"));

    plan.apply().expect("apply");
    let loaded_user = UserConfig::load_exact(&config_path).expect("user config");
    assert_eq!(loaded_user, plan.user_config);

    let state = InstallState::load(&state_path).expect("state");
    assert_eq!(state.components.len(), 3);
    assert_eq!(state.config_path, config_path);
    assert!(state.config_digest.is_some());
    assert!(state.matches_user_config(&config_path, &loaded_user));

    let resolved = loaded_user
        .resolve(None, Some(&state))
        .expect("resolved config");
    assert_eq!(resolved, plan.config);
}

#[test]
fn automatic_external_selection_falls_back_when_presenter_is_missing() {
    let root = tempfile::tempdir().expect("temp root");
    let mmdc = program_path(root.path(), "resolved", "mmdc");
    let resolver = FakeResolver::default().with("mmdc", mmdc);
    let installer = Installer::new(resolver);
    let request = InstallRequest::new(
        root.path().join("config.toml"),
        root.path().join("state.toml"),
    );

    let plan = installer.plan(&request).expect("plan");
    assert_eq!(plan.config.engines.mermaid.backend, MermaidEngine::Preview);
    assert!(
        plan.warnings
            .iter()
            .any(|warning| warning.contains("presenter"))
    );
}

#[test]
fn explicitly_requested_missing_engine_is_an_error() {
    let root = tempfile::tempdir().expect("temp root");
    let installer = Installer::new(FakeResolver::default());
    let mut request = InstallRequest::new(
        root.path().join("config.toml"),
        root.path().join("state.toml"),
    );
    request.mermaid = EnginePreference::External(program_path(root.path(), "missing", "mmdc"));
    request.presenter = PresenterPreference::Program(program_path(root.path(), "missing", "chafa"));

    let error = installer
        .plan(&request)
        .expect_err("missing explicit engine");
    assert!(error.to_string().contains("cannot select mermaid"));
}

#[test]
fn rerun_replaces_one_engine_without_resetting_other_user_settings() {
    let root = tempfile::tempdir().expect("temp root");
    let config_path = root.path().join("config.toml");
    let state_path = root.path().join("state.toml");
    let old_mmdc = program_path(root.path(), "old", "mmdc");
    let old_chafa = program_path(root.path(), "old", "chafa");
    let new_mmdc = program_path(root.path(), "new", "mmdc");

    let mut existing = UserConfig::default();
    let profile = existing.profile_mut("default").expect("profile");
    profile.detection.math = false;
    profile.engines.mermaid = EngineSelection {
        provider: EngineProvider::External,
        program: Some(old_mmdc),
    };
    profile.engines.math = EngineSelection {
        provider: EngineProvider::Source,
        program: None,
    };
    profile.engines.presenter = PresenterSelection {
        provider: PresenterProvider::External,
        program: Some(old_chafa.clone()),
    };
    fs::write(&config_path, existing.to_toml().expect("serialize")).expect("write config");

    let resolver = FakeResolver::default()
        .with(&new_mmdc, &new_mmdc)
        .with(&old_chafa, &old_chafa);
    let installer = Installer::new(resolver);
    let mut request = InstallRequest::new(config_path.clone(), state_path);
    request.mermaid = EnginePreference::External(new_mmdc.clone());

    let plan = installer.plan(&request).expect("replace plan");
    assert!(!plan.config.detection.math);
    assert_eq!(plan.config.engines.math.backend, MathEngine::Source);
    assert_eq!(
        plan.config.engines.mermaid.backend,
        MermaidEngine::MermaidCli
    );
    assert_eq!(plan.config.engines.mermaid.path, new_mmdc);
    assert_eq!(plan.config.engines.presenter.path, old_chafa);
    assert_eq!(
        plan.user_config.profiles["default"].engines.math.provider,
        EngineProvider::Source
    );
}

#[test]
fn reset_discards_existing_engine_choices_but_keeps_resolution_extensible() {
    let root = tempfile::tempdir().expect("temp root");
    let config_path = root.path().join("config.toml");
    let mut existing = UserConfig::default();
    existing
        .profile_mut("default")
        .expect("profile")
        .engines
        .math = EngineSelection {
        provider: EngineProvider::Source,
        program: None,
    };
    fs::write(&config_path, existing.to_toml().expect("serialize")).expect("write config");

    let resolver = FakeResolver::default();
    let installer = Installer::new(resolver);
    let mut request = InstallRequest::new(config_path, root.path().join("state.toml"));
    request.reset = true;
    request.mermaid = EnginePreference::Preview;
    request.math = EnginePreference::Preview;

    let plan = installer.plan(&request).expect("reset plan");
    assert_eq!(plan.config.engines.math.backend, MathEngine::Preview);
    assert_eq!(
        plan.user_config.profiles["default"].engines.math.provider,
        EngineProvider::Preview
    );
}
