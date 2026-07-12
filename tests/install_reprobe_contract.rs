use ptymark::{
    Config, EnginePreference, InstallError, InstallRequest, Installer, MathEngine,
    PathProgramResolver, PresenterPreference, ProgramResolver,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

fn temp_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ptymark-reprobe-{nonce}"));
    fs::create_dir_all(&root).expect("temp root");
    root
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
fn auto_reprobes_standard_names_instead_of_reusing_stale_absolute_paths() {
    let root = temp_root();
    let config_path = root.join("config.toml");
    let state_path = root.join("install.toml");
    let old_mmdc = program_path(&root, "old", "mmdc");
    let old_chafa = program_path(&root, "old", "chafa");
    let new_mmdc = program_path(&root, "new", "mmdc");
    let new_chafa = program_path(&root, "new", "chafa");

    let mut existing = Config::default();
    existing.engines.mermaid.backend = ptymark::MermaidEngine::MermaidCli;
    existing.engines.mermaid.path = old_mmdc;
    existing.engines.math.backend = MathEngine::Preview;
    existing.engines.presenter.path = old_chafa;
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
    let _ = fs::remove_dir_all(root);
}

#[test]
fn public_path_resolver_remains_constructible_for_default_installer_composition() {
    let _installer = Installer::new(PathProgramResolver);
}
