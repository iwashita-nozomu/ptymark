use ptymark::{
    EnginePreference, EngineProvider, EngineSelection, InstallError, InstallRequest, Installer,
    PathProgramResolver, PresenterPreference, PresenterProvider, PresenterSelection,
    ProgramResolver, UserConfig,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct MapResolver {
    paths: HashMap<PathBuf, PathBuf>,
}

impl MapResolver {
    fn with(mut self, requested: impl Into<PathBuf>, resolved: impl Into<PathBuf>) -> Self {
        self.paths.insert(requested.into(), resolved.into());
        self
    }
}

impl ProgramResolver for MapResolver {
    fn resolve(&self, configured: &Path) -> Result<PathBuf, InstallError> {
        self.paths
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
fn auto_reprobes_standard_names_without_persisting_stale_paths() {
    let root = tempfile::tempdir().expect("temp root");
    let config_path = root.path().join("config.toml");
    let state_path = root.path().join("install.toml");
    let old_mmdc = program_path(root.path(), "old", "mmdc");
    let old_chafa = program_path(root.path(), "old", "chafa");
    let new_mmdc = program_path(root.path(), "new", "mmdc");
    let new_chafa = program_path(root.path(), "new", "chafa");

    let mut existing = UserConfig::default();
    let profile = existing.profile_mut("default").expect("profile");
    profile.engines.mermaid = EngineSelection {
        provider: EngineProvider::External,
        program: Some(old_mmdc),
    };
    profile.engines.presenter = PresenterSelection {
        provider: PresenterProvider::External,
        program: Some(old_chafa),
    };
    fs::write(&config_path, existing.to_toml().expect("serialize")).expect("write config");

    let resolver = MapResolver::default()
        .with("mmdc", &new_mmdc)
        .with("chafa", &new_chafa);
    let installer = Installer::new(resolver);
    let mut request = InstallRequest::new(config_path, state_path);
    request.mermaid = EnginePreference::Auto;
    request.math = EnginePreference::Preview;
    request.presenter = PresenterPreference::Auto;

    let plan = installer.plan(&request).expect("re-probe plan");
    assert_eq!(plan.config.engines.mermaid.path, new_mmdc);
    assert_eq!(plan.config.engines.presenter.path, new_chafa);
    assert_eq!(
        plan.user_config.profiles["default"]
            .engines
            .mermaid
            .provider,
        EngineProvider::Auto
    );
    let source = plan.user_config.to_toml().expect("TOML");
    assert!(!source.contains("old"));
    assert!(!source.contains("new"));
}

#[test]
fn public_path_resolver_remains_constructible_for_default_installer_composition() {
    let _installer = Installer::new(PathProgramResolver);
}
