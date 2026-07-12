use crate::config::{Config, MathEngine, MermaidEngine};
use crate::engine::resolve_executable;
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const INSTALL_STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnginePreference {
    Keep,
    Auto,
    Preview,
    Source,
    External(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresenterPreference {
    Keep,
    Auto,
    Program(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallRequest {
    pub config_path: PathBuf,
    pub state_path: PathBuf,
    pub mermaid: EnginePreference,
    pub math: EnginePreference,
    pub presenter: PresenterPreference,
    pub reset: bool,
}

impl InstallRequest {
    pub fn new(config_path: PathBuf, state_path: PathBuf) -> Self {
        Self {
            config_path,
            state_path,
            mermaid: EnginePreference::Keep,
            math: EnginePreference::Keep,
            presenter: PresenterPreference::Keep,
            reset: false,
        }
    }
}

pub trait ProgramResolver: Send + Sync {
    fn resolve(&self, configured: &Path) -> Result<PathBuf, InstallError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PathProgramResolver;

impl ProgramResolver for PathProgramResolver {
    fn resolve(&self, configured: &Path) -> Result<PathBuf, InstallError> {
        resolve_executable(configured).map_err(|error| InstallError::new(error.to_string()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InstallState {
    pub schema_version: u32,
    pub ptymark_version: String,
    pub config_path: PathBuf,
    #[serde(default)]
    pub components: Vec<InstalledComponent>,
}

impl InstallState {
    pub fn load(path: &Path) -> Result<Self, InstallError> {
        let source = fs::read_to_string(path).map_err(|error| {
            InstallError::new(format!(
                "cannot read installation state `{}`: {error}",
                path.display()
            ))
        })?;
        let state: Self = toml::from_str(&source).map_err(|error| {
            InstallError::new(format!(
                "cannot parse installation state `{}`: {error}",
                path.display()
            ))
        })?;
        if state.schema_version != INSTALL_STATE_SCHEMA_VERSION {
            return Err(InstallError::new(format!(
                "unsupported installation state schema {}; expected {}",
                state.schema_version, INSTALL_STATE_SCHEMA_VERSION
            )));
        }
        Ok(state)
    }

    pub fn to_toml(&self) -> Result<String, InstallError> {
        toml::to_string_pretty(self).map_err(|error| {
            InstallError::new(format!("cannot serialize installation state: {error}"))
        })
    }

    pub fn status_lines(&self, resolver: &dyn ProgramResolver) -> Vec<String> {
        let mut lines = vec![format!("config\t{}", self.config_path.display())];
        for component in &self.components {
            let status = match component.resolved_path.as_deref() {
                Some(path) if resolver.resolve(path).is_ok() => "ready",
                Some(_) => "missing",
                None if component.active => "built-in",
                None => "inactive",
            };
            let resolved = component
                .resolved_path
                .as_deref()
                .map_or_else(|| "-".to_owned(), |path| path.display().to_string());
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                component.role, component.backend, status, resolved
            ));
        }
        lines
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InstalledComponent {
    pub role: String,
    pub backend: String,
    pub active: bool,
    pub origin: ResolutionOrigin,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResolutionOrigin {
    BuiltIn,
    Existing,
    PathSearch,
    Explicit,
    AutoFallback,
}

#[derive(Clone, Debug)]
pub struct InstallPlan {
    pub config_path: PathBuf,
    pub state_path: PathBuf,
    pub config: Config,
    pub state: InstallState,
    pub warnings: Vec<String>,
}

impl InstallPlan {
    pub fn apply(&self) -> Result<(), InstallError> {
        atomic_write(&self.config_path, self.config.to_toml()?.as_bytes())?;
        atomic_write(&self.state_path, self.state.to_toml()?.as_bytes())?;
        Ok(())
    }

    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("config\t{}", self.config_path.display()),
            format!("state\t{}", self.state_path.display()),
        ];
        lines.extend(self.state.components.iter().map(|component| {
            let resolved = component
                .resolved_path
                .as_deref()
                .map_or_else(|| "built-in".to_owned(), |path| path.display().to_string());
            format!("{}\t{}\t{}", component.role, component.backend, resolved)
        }));
        lines.extend(
            self.warnings
                .iter()
                .map(|warning| format!("warning\t{warning}")),
        );
        lines
    }
}

#[derive(Clone, Copy, Debug)]
struct SlotSpec {
    role: &'static str,
    default_program: &'static str,
    external_backend: &'static str,
}

const MERMAID_SLOT: SlotSpec = SlotSpec {
    role: "mermaid",
    default_program: "mmdc",
    external_backend: "mermaid-cli",
};

const MATH_SLOT: SlotSpec = SlotSpec {
    role: "math",
    default_program: "tex2svg",
    external_backend: "mathjax-cli",
};

pub struct Installer<R> {
    resolver: R,
}

impl<R: ProgramResolver> Installer<R> {
    pub const fn new(resolver: R) -> Self {
        Self { resolver }
    }

    pub fn plan(&self, request: &InstallRequest) -> Result<InstallPlan, InstallError> {
        let config_path = absolute_path(&request.config_path)?;
        let state_path = absolute_path(&request.state_path)?;
        let existing = config_path.is_file() && !request.reset;
        let mut config = if existing {
            Config::load_exact(&config_path)?
        } else {
            Config::default()
        };
        let mut warnings = Vec::new();

        let mut mermaid = self.plan_slot(
            MERMAID_SLOT,
            request.mermaid.clone(),
            existing,
            ExistingSlot::from_mermaid(&config),
            &mut warnings,
        )?;
        let mut math = self.plan_slot(
            MATH_SLOT,
            request.math.clone(),
            existing,
            ExistingSlot::from_math(&config),
            &mut warnings,
        )?;

        let any_external =
            mermaid.route == SlotRoute::External || math.route == SlotRoute::External;
        let required_external = (mermaid.route == SlotRoute::External && mermaid.required)
            || (math.route == SlotRoute::External && math.required);
        let presenter = self.plan_presenter(
            request.presenter.clone(),
            existing,
            config.engines.presenter.path.clone(),
            any_external,
            required_external,
            &mut warnings,
        )?;

        if any_external && presenter.resolved.is_none() {
            mermaid.fallback_if_optional("Chafa presenter was not found", &mut warnings);
            math.fallback_if_optional("Chafa presenter was not found", &mut warnings);
        }

        apply_mermaid(&mut config, &mermaid);
        apply_math(&mut config, &math);
        if let Some(path) = presenter.resolved.as_ref() {
            config.engines.presenter.path = path.clone();
        }
        config.validate()?;

        let components = vec![
            mermaid.into_component(),
            math.into_component(),
            presenter.into_component(),
        ];
        let state = InstallState {
            schema_version: INSTALL_STATE_SCHEMA_VERSION,
            ptymark_version: env!("CARGO_PKG_VERSION").to_owned(),
            config_path: config_path.clone(),
            components,
        };

        Ok(InstallPlan {
            config_path,
            state_path,
            config,
            state,
            warnings,
        })
    }

    fn plan_slot(
        &self,
        spec: SlotSpec,
        preference: EnginePreference,
        existing_config: bool,
        existing: ExistingSlot,
        warnings: &mut Vec<String>,
    ) -> Result<SlotPlan, InstallError> {
        let role = spec.role;
        let external_backend = spec.external_backend;
        let default_program = PathBuf::from(spec.default_program);
        match preference {
            EnginePreference::Keep if existing_config => match existing.route {
                SlotRoute::Preview => Ok(SlotPlan::builtin(
                    role,
                    external_backend,
                    SlotRoute::Preview,
                    ResolutionOrigin::Existing,
                )),
                SlotRoute::Source => Ok(SlotPlan::builtin(
                    role,
                    external_backend,
                    SlotRoute::Source,
                    ResolutionOrigin::Existing,
                )),
                SlotRoute::External => {
                    self.required_external(role, external_backend, existing.path, false)
                }
            },
            EnginePreference::Keep | EnginePreference::Auto => {
                let candidate = if existing_config {
                    existing.path
                } else {
                    default_program
                };
                match self.resolver.resolve(&candidate) {
                    Ok(resolved) => Ok(SlotPlan::external(
                        role,
                        external_backend,
                        candidate,
                        resolved,
                        false,
                        if existing_config {
                            ResolutionOrigin::Existing
                        } else {
                            ResolutionOrigin::PathSearch
                        },
                    )),
                    Err(error) => {
                        warnings.push(format!(
                            "{role} external engine was not selected: {error}; using preview"
                        ));
                        Ok(SlotPlan::fallback(
                            role,
                            external_backend,
                            candidate,
                            error.to_string(),
                        ))
                    }
                }
            }
            EnginePreference::Preview => Ok(SlotPlan::builtin(
                role,
                external_backend,
                SlotRoute::Preview,
                ResolutionOrigin::Explicit,
            )),
            EnginePreference::Source => Ok(SlotPlan::builtin(
                role,
                external_backend,
                SlotRoute::Source,
                ResolutionOrigin::Explicit,
            )),
            EnginePreference::External(path) => {
                self.required_external(role, external_backend, path, true)
            }
        }
    }

    fn required_external(
        &self,
        role: &'static str,
        backend: &'static str,
        configured: PathBuf,
        explicit: bool,
    ) -> Result<SlotPlan, InstallError> {
        let resolved = self.resolver.resolve(&configured).map_err(|error| {
            InstallError::new(format!(
                "cannot select {role} backend `{backend}` from `{}`: {error}",
                configured.display()
            ))
        })?;
        Ok(SlotPlan::external(
            role,
            backend,
            configured,
            resolved,
            true,
            if explicit {
                ResolutionOrigin::Explicit
            } else {
                ResolutionOrigin::Existing
            },
        ))
    }

    fn plan_presenter(
        &self,
        preference: PresenterPreference,
        existing_config: bool,
        existing_path: PathBuf,
        required: bool,
        required_external: bool,
        warnings: &mut Vec<String>,
    ) -> Result<PresenterPlan, InstallError> {
        if !required {
            return match preference {
                PresenterPreference::Program(path) => {
                    let resolved = self.resolver.resolve(&path).map_err(|error| {
                        InstallError::new(format!(
                            "cannot select presenter from `{}`: {error}",
                            path.display()
                        ))
                    })?;
                    Ok(PresenterPlan::resolved(
                        path,
                        resolved,
                        ResolutionOrigin::Explicit,
                    ))
                }
                PresenterPreference::Keep if existing_config => Ok(PresenterPlan::inactive(
                    existing_path,
                    ResolutionOrigin::Existing,
                )),
                PresenterPreference::Keep | PresenterPreference::Auto => Ok(
                    PresenterPlan::inactive(PathBuf::from("chafa"), ResolutionOrigin::BuiltIn),
                ),
            };
        }

        let (candidate, must_resolve, origin) = match preference {
            PresenterPreference::Keep if existing_config => {
                (existing_path, true, ResolutionOrigin::Existing)
            }
            PresenterPreference::Keep | PresenterPreference::Auto => (
                if existing_config {
                    existing_path
                } else {
                    PathBuf::from("chafa")
                },
                required_external,
                if existing_config {
                    ResolutionOrigin::Existing
                } else {
                    ResolutionOrigin::PathSearch
                },
            ),
            PresenterPreference::Program(path) => (path, true, ResolutionOrigin::Explicit),
        };

        match self.resolver.resolve(&candidate) {
            Ok(resolved) => Ok(PresenterPlan::resolved(candidate, resolved, origin)),
            Err(error) if must_resolve => Err(InstallError::new(format!(
                "cannot select Chafa presenter from `{}`: {error}",
                candidate.display()
            ))),
            Err(error) => {
                warnings.push(format!(
                    "external engines were not activated because the presenter could not be resolved: {error}"
                ));
                Ok(PresenterPlan::missing(candidate, error.to_string()))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SlotRoute {
    Preview,
    Source,
    External,
}

#[derive(Clone, Debug)]
struct ExistingSlot {
    route: SlotRoute,
    path: PathBuf,
}

impl ExistingSlot {
    fn from_mermaid(config: &Config) -> Self {
        let route = match config.engines.mermaid.backend {
            MermaidEngine::Preview => SlotRoute::Preview,
            MermaidEngine::Source => SlotRoute::Source,
            MermaidEngine::MermaidCli => SlotRoute::External,
        };
        Self {
            route,
            path: config.engines.mermaid.path.clone(),
        }
    }

    fn from_math(config: &Config) -> Self {
        let route = match config.engines.math.backend {
            MathEngine::Preview => SlotRoute::Preview,
            MathEngine::Source => SlotRoute::Source,
            MathEngine::MathjaxCli => SlotRoute::External,
        };
        Self {
            route,
            path: config.engines.math.path.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct SlotPlan {
    role: &'static str,
    external_backend: &'static str,
    route: SlotRoute,
    requested: Option<PathBuf>,
    resolved: Option<PathBuf>,
    required: bool,
    origin: ResolutionOrigin,
    note: Option<String>,
}

impl SlotPlan {
    fn builtin(
        role: &'static str,
        external_backend: &'static str,
        route: SlotRoute,
        origin: ResolutionOrigin,
    ) -> Self {
        Self {
            role,
            external_backend,
            route,
            requested: None,
            resolved: None,
            required: false,
            origin,
            note: None,
        }
    }

    fn external(
        role: &'static str,
        external_backend: &'static str,
        requested: PathBuf,
        resolved: PathBuf,
        required: bool,
        origin: ResolutionOrigin,
    ) -> Self {
        Self {
            role,
            external_backend,
            route: SlotRoute::External,
            requested: Some(requested),
            resolved: Some(resolved),
            required,
            origin,
            note: None,
        }
    }

    fn fallback(
        role: &'static str,
        external_backend: &'static str,
        requested: PathBuf,
        reason: String,
    ) -> Self {
        Self {
            role,
            external_backend,
            route: SlotRoute::Preview,
            requested: Some(requested),
            resolved: None,
            required: false,
            origin: ResolutionOrigin::AutoFallback,
            note: Some(reason),
        }
    }

    fn fallback_if_optional(&mut self, reason: &str, warnings: &mut Vec<String>) {
        if self.route == SlotRoute::External && !self.required {
            warnings.push(format!(
                "{} external engine was resolved but not activated: {reason}; using preview",
                self.role
            ));
            self.route = SlotRoute::Preview;
            self.origin = ResolutionOrigin::AutoFallback;
            self.note = Some(reason.to_owned());
            self.resolved = None;
        }
    }

    fn backend(&self) -> &'static str {
        match self.route {
            SlotRoute::Preview => "preview",
            SlotRoute::Source => "source",
            SlotRoute::External => self.external_backend,
        }
    }

    fn into_component(self) -> InstalledComponent {
        InstalledComponent {
            role: self.role.to_owned(),
            backend: self.backend().to_owned(),
            active: true,
            origin: self.origin,
            requested_path: self.requested,
            resolved_path: self.resolved,
            note: self.note,
        }
    }
}

#[derive(Clone, Debug)]
struct PresenterPlan {
    requested: PathBuf,
    resolved: Option<PathBuf>,
    active: bool,
    origin: ResolutionOrigin,
    note: Option<String>,
}

impl PresenterPlan {
    fn resolved(requested: PathBuf, resolved: PathBuf, origin: ResolutionOrigin) -> Self {
        Self {
            requested,
            resolved: Some(resolved),
            active: true,
            origin,
            note: None,
        }
    }

    fn inactive(requested: PathBuf, origin: ResolutionOrigin) -> Self {
        Self {
            requested,
            resolved: None,
            active: false,
            origin,
            note: None,
        }
    }

    fn missing(requested: PathBuf, reason: String) -> Self {
        Self {
            requested,
            resolved: None,
            active: false,
            origin: ResolutionOrigin::AutoFallback,
            note: Some(reason),
        }
    }

    fn into_component(self) -> InstalledComponent {
        InstalledComponent {
            role: "presenter".to_owned(),
            backend: if self.active {
                "chafa-symbols".to_owned()
            } else {
                "unused".to_owned()
            },
            active: self.active,
            origin: self.origin,
            requested_path: Some(self.requested),
            resolved_path: self.resolved,
            note: self.note,
        }
    }
}

fn apply_mermaid(config: &mut Config, plan: &SlotPlan) {
    config.engines.mermaid.backend = match plan.route {
        SlotRoute::Preview => MermaidEngine::Preview,
        SlotRoute::Source => MermaidEngine::Source,
        SlotRoute::External => MermaidEngine::MermaidCli,
    };
    if let Some(path) = plan.resolved.as_ref() {
        config.engines.mermaid.path = path.clone();
    }
}

fn apply_math(config: &mut Config, plan: &SlotPlan) {
    config.engines.math.backend = match plan.route {
        SlotRoute::Preview => MathEngine::Preview,
        SlotRoute::Source => MathEngine::Source,
        SlotRoute::External => MathEngine::MathjaxCli,
    };
    if let Some(path) = plan.resolved.as_ref() {
        config.engines.math.path = path.clone();
    }
}

pub fn default_install_state_path() -> Result<PathBuf, InstallError> {
    if let Some(path) = env::var_os("PTYMARK_INSTALL_STATE") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(path).join("ptymark/install.toml"));
    }
    let home = home_dir().ok_or_else(|| {
        InstallError::new(
            "cannot determine installation state path; set HOME, XDG_STATE_HOME, or PTYMARK_INSTALL_STATE",
        )
    })?;
    Ok(home.join(".local/state/ptymark/install.toml"))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn absolute_path(path: &Path) -> Result<PathBuf, InstallError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    env::current_dir()
        .map(|directory| directory.join(path))
        .map_err(|error| InstallError::new(format!("cannot resolve current directory: {error}")))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), InstallError> {
    let parent = path.parent().ok_or_else(|| {
        InstallError::new(format!("path `{}` has no parent directory", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        InstallError::new(format!(
            "cannot create installation directory `{}`: {error}",
            parent.display()
        ))
    })?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("ptymark");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for attempt in 0..32_u32 {
        let temporary = parent.join(format!(
            ".{file_name}.tmp-{}-{timestamp}-{attempt}",
            std::process::id()
        ));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(InstallError::new(format!(
                    "cannot create temporary installation file `{}`: {error}",
                    temporary.display()
                )));
            }
        };
        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            let _ = fs::remove_file(&temporary);
            return Err(InstallError::new(format!(
                "cannot write temporary installation file `{}`: {error}",
                temporary.display()
            )));
        }
        drop(file);
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::remove_file(&temporary);
            return Err(InstallError::new(format!(
                "cannot replace installation file `{}`: {error}",
                path.display()
            )));
        }
        return Ok(());
    }

    Err(InstallError::new(format!(
        "cannot allocate a temporary file for `{}`",
        path.display()
    )))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallError {
    message: String,
}

impl InstallError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for InstallError {}

impl From<crate::config::ConfigError> for InstallError {
    fn from(error: crate::config::ConfigError) -> Self {
        Self::new(error.to_string())
    }
}
