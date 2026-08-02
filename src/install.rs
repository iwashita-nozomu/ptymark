use crate::config::{
    Config, EngineProvider, EngineSelection, MathEngine, MermaidEngine, PresenterProvider,
    PresenterSelection, UserConfig,
};
use crate::engine::resolve_executable;
use crate::managed_launcher::inspect_managed_alias;
use crate::platform::PlatformPaths;
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const INSTALL_STATE_SCHEMA_VERSION: u32 = 2;
pub const LEGACY_INSTALL_STATE_SCHEMA_VERSION: u32 = 1;

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
#[serde(deny_unknown_fields)]
pub struct InstallState {
    pub schema_version: u32,
    pub ptymark_version: String,
    pub config_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_digest: Option<String>,
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
        if !matches!(
            state.schema_version,
            LEGACY_INSTALL_STATE_SCHEMA_VERSION | INSTALL_STATE_SCHEMA_VERSION
        ) {
            return Err(InstallError::new(format!(
                "unsupported installation state schema {}; expected {} or {}",
                state.schema_version,
                LEGACY_INSTALL_STATE_SCHEMA_VERSION,
                INSTALL_STATE_SCHEMA_VERSION
            )));
        }
        if state.schema_version == INSTALL_STATE_SCHEMA_VERSION && state.config_digest.is_none() {
            return Err(InstallError::new(
                "installation state schema 2 requires config_digest",
            ));
        }
        Ok(state)
    }

    pub fn to_toml(&self) -> Result<String, InstallError> {
        toml::to_string_pretty(self).map_err(|error| {
            InstallError::new(format!("cannot serialize installation state: {error}"))
        })
    }

    pub fn matches_user_config(&self, config_path: &Path, user: &UserConfig) -> bool {
        if self.config_path != config_path {
            return false;
        }
        match self.config_digest.as_deref() {
            Some(expected) => user.fingerprint().is_ok_and(|actual| actual == expected),
            None => self.schema_version == LEGACY_INSTALL_STATE_SCHEMA_VERSION,
        }
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
    Managed,
    AutoFallback,
}

#[derive(Clone, Debug)]
pub struct InstallPlan {
    pub config_path: PathBuf,
    pub state_path: PathBuf,
    /// Resolved runtime view for inspection and tests. This is never serialized
    /// into the user configuration file.
    pub config: Config,
    pub user_config: UserConfig,
    pub state: InstallState,
    pub warnings: Vec<String>,
}

impl InstallPlan {
    pub fn apply(&self) -> Result<(), InstallError> {
        let user_toml = self.user_config.to_toml()?;
        let expected_digest = self.user_config.fingerprint()?;
        if self.state.config_digest.as_deref() != Some(expected_digest.as_str()) {
            return Err(InstallError::new(
                "installation plan state does not match the user configuration digest",
            ));
        }
        let state_toml = self.state.to_toml()?;
        let previous_state = fs::read(&self.state_path).ok();

        // Commit state first. If the process stops before the config commit,
        // runtime resolution rejects the state because its digest does not
        // match the still-active configuration.
        atomic_replace(&self.state_path, state_toml.as_bytes())?;
        if let Err(error) = atomic_replace(&self.config_path, user_toml.as_bytes()) {
            let rollback = restore_previous(&self.state_path, previous_state.as_deref());
            return Err(match rollback {
                Ok(()) => InstallError::new(format!(
                    "cannot commit user configuration; installation state was rolled back: {error}"
                )),
                Err(rollback_error) => InstallError::new(format!(
                    "cannot commit user configuration: {error}; installation-state rollback also failed: {rollback_error}"
                )),
            });
        }
        Ok(())
    }

    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("config\t{}", self.config_path.display()),
            format!("state\t{}", self.state_path.display()),
            format!("profile\t{}", self.config.selected_profile),
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
        let existing_state = state_path
            .is_file()
            .then(|| InstallState::load(&state_path))
            .transpose()?
            .filter(|state| state.config_path == config_path);
        let mut user_config = if existing {
            UserConfig::load_exact(&config_path)?
        } else {
            UserConfig::default()
        };
        let selected_profile = user_config.default_profile.clone();
        let existing_profile = user_config
            .profiles
            .get(&selected_profile)
            .cloned()
            .ok_or_else(|| InstallError::new("selected profile is missing"))?;
        let mut warnings = Vec::new();

        let mut mermaid = self.plan_slot(
            MERMAID_SLOT,
            request.mermaid.clone(),
            existing,
            existing_profile.engines.mermaid,
            existing_state.as_ref(),
            &mut warnings,
        )?;
        let mut math = self.plan_slot(
            MATH_SLOT,
            request.math.clone(),
            existing,
            existing_profile.engines.math,
            existing_state.as_ref(),
            &mut warnings,
        )?;

        let any_external =
            mermaid.route == SlotRoute::External || math.route == SlotRoute::External;
        let required_external = (mermaid.route == SlotRoute::External && mermaid.required)
            || (math.route == SlotRoute::External && math.required);
        let presenter = self.plan_presenter(
            request.presenter.clone(),
            existing,
            existing_profile.engines.presenter,
            existing_state.as_ref(),
            any_external,
            required_external,
            &mut warnings,
        )?;

        if any_external && presenter.resolved.is_none() {
            mermaid.fallback_if_optional("Chafa presenter was not found", &mut warnings);
            math.fallback_if_optional("Chafa presenter was not found", &mut warnings);
        }

        {
            let profile = user_config.profile_mut(&selected_profile)?;
            profile.engines.mermaid = mermaid.selection.clone();
            profile.engines.math = math.selection.clone();
            profile.engines.presenter = presenter.selection.clone();
        }
        user_config.validate()?;

        let components = vec![
            mermaid.into_component(),
            math.into_component(),
            presenter.into_component(),
        ];
        let state = InstallState {
            schema_version: INSTALL_STATE_SCHEMA_VERSION,
            ptymark_version: env!("CARGO_PKG_VERSION").to_owned(),
            config_path: config_path.clone(),
            config_digest: Some(user_config.fingerprint()?),
            components,
        };
        let config = user_config.resolve(None, Some(&state))?;

        Ok(InstallPlan {
            config_path,
            state_path,
            config,
            user_config,
            state,
            warnings,
        })
    }

    fn plan_slot(
        &self,
        spec: SlotSpec,
        preference: EnginePreference,
        existing_config: bool,
        existing_selection: EngineSelection,
        existing_state: Option<&InstallState>,
        warnings: &mut Vec<String>,
    ) -> Result<SlotPlan, InstallError> {
        match preference {
            EnginePreference::Keep if existing_config => {
                self.plan_existing_slot(spec, existing_selection, existing_state, warnings)
            }
            EnginePreference::Keep | EnginePreference::Auto => self.plan_auto_slot(spec, warnings),
            EnginePreference::Preview => Ok(SlotPlan::builtin(
                spec,
                SlotRoute::Preview,
                ResolutionOrigin::Explicit,
                EngineSelection {
                    provider: EngineProvider::Preview,
                    program: None,
                },
            )),
            EnginePreference::Source => Ok(SlotPlan::builtin(
                spec,
                SlotRoute::Source,
                ResolutionOrigin::Explicit,
                EngineSelection {
                    provider: EngineProvider::Source,
                    program: None,
                },
            )),
            EnginePreference::External(path) => self.required_external(spec, path, true),
        }
    }

    fn plan_existing_slot(
        &self,
        spec: SlotSpec,
        selection: EngineSelection,
        state: Option<&InstallState>,
        warnings: &mut Vec<String>,
    ) -> Result<SlotPlan, InstallError> {
        match selection.provider {
            EngineProvider::Auto => self.plan_auto_slot(spec, warnings),
            EngineProvider::Preview => Ok(SlotPlan::builtin(
                spec,
                SlotRoute::Preview,
                ResolutionOrigin::Existing,
                selection,
            )),
            EngineProvider::Source => Ok(SlotPlan::builtin(
                spec,
                SlotRoute::Source,
                ResolutionOrigin::Existing,
                selection,
            )),
            EngineProvider::External => self.required_external(
                spec,
                selection
                    .program
                    .clone()
                    .ok_or_else(|| InstallError::new("external provider has no program"))?,
                false,
            ),
            EngineProvider::Managed => {
                let path = state_program(state, spec.role).ok_or_else(|| {
                    InstallError::new(format!(
                        "existing profile requires managed {}, but matching installation state is unavailable",
                        spec.role
                    ))
                })?;
                let resolved = self.resolver.resolve(&path)?;
                Ok(SlotPlan::external(
                    spec,
                    path,
                    resolved,
                    true,
                    ResolutionOrigin::Managed,
                    selection,
                ))
            }
        }
    }

    fn plan_auto_slot(
        &self,
        spec: SlotSpec,
        warnings: &mut Vec<String>,
    ) -> Result<SlotPlan, InstallError> {
        let candidate = PathBuf::from(spec.default_program);
        let selection = EngineSelection::default();
        match self.resolver.resolve(&candidate) {
            Ok(resolved) => Ok(SlotPlan::external(
                spec,
                candidate,
                resolved,
                false,
                ResolutionOrigin::PathSearch,
                selection,
            )),
            Err(error) => {
                warnings.push(format!(
                    "{} external engine was not selected: {error}; using preview",
                    spec.role
                ));
                Ok(SlotPlan::fallback(
                    spec,
                    candidate,
                    error.to_string(),
                    selection,
                ))
            }
        }
    }

    fn required_external(
        &self,
        spec: SlotSpec,
        configured: PathBuf,
        explicit: bool,
    ) -> Result<SlotPlan, InstallError> {
        let resolved = self.resolver.resolve(&configured).map_err(|error| {
            InstallError::new(format!(
                "cannot select {} backend `{}` from `{}`: {error}",
                spec.role,
                spec.external_backend,
                configured.display()
            ))
        })?;
        let managed = inspect_managed_alias(&resolved).is_some();
        let selection = if managed {
            EngineSelection {
                provider: EngineProvider::Managed,
                program: None,
            }
        } else {
            EngineSelection {
                provider: EngineProvider::External,
                program: Some(configured.clone()),
            }
        };
        Ok(SlotPlan::external(
            spec,
            configured,
            resolved,
            true,
            if managed {
                ResolutionOrigin::Managed
            } else if explicit {
                ResolutionOrigin::Explicit
            } else {
                ResolutionOrigin::Existing
            },
            selection,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn plan_presenter(
        &self,
        preference: PresenterPreference,
        existing_config: bool,
        existing_selection: PresenterSelection,
        existing_state: Option<&InstallState>,
        required: bool,
        required_external: bool,
        warnings: &mut Vec<String>,
    ) -> Result<PresenterPlan, InstallError> {
        let selection = match preference {
            PresenterPreference::Keep if existing_config => existing_selection,
            PresenterPreference::Keep | PresenterPreference::Auto => PresenterSelection::default(),
            PresenterPreference::Program(path) => {
                let resolved = self.resolver.resolve(&path).map_err(|error| {
                    InstallError::new(format!(
                        "cannot select presenter from `{}`: {error}",
                        path.display()
                    ))
                })?;
                let managed = inspect_managed_alias(&resolved).is_some();
                return Ok(if required {
                    PresenterPlan::resolved(
                        path.clone(),
                        resolved,
                        if managed {
                            ResolutionOrigin::Managed
                        } else {
                            ResolutionOrigin::Explicit
                        },
                        if managed {
                            PresenterSelection {
                                provider: PresenterProvider::Managed,
                                program: None,
                            }
                        } else {
                            PresenterSelection {
                                provider: PresenterProvider::External,
                                program: Some(path),
                            }
                        },
                    )
                } else {
                    PresenterPlan::inactive(
                        path.clone(),
                        if managed {
                            ResolutionOrigin::Managed
                        } else {
                            ResolutionOrigin::Explicit
                        },
                        if managed {
                            PresenterSelection {
                                provider: PresenterProvider::Managed,
                                program: None,
                            }
                        } else {
                            PresenterSelection {
                                provider: PresenterProvider::External,
                                program: Some(path),
                            }
                        },
                    )
                });
            }
        };

        if !required {
            return Ok(PresenterPlan::inactive(
                PathBuf::from("chafa"),
                if existing_config {
                    ResolutionOrigin::Existing
                } else {
                    ResolutionOrigin::BuiltIn
                },
                selection,
            ));
        }

        let (candidate, must_resolve, origin) = match selection.provider {
            PresenterProvider::Auto => (
                PathBuf::from("chafa"),
                required_external,
                ResolutionOrigin::PathSearch,
            ),
            PresenterProvider::External => (
                selection
                    .program
                    .clone()
                    .ok_or_else(|| InstallError::new("external presenter has no program"))?,
                true,
                ResolutionOrigin::Existing,
            ),
            PresenterProvider::Managed => (
                state_program(existing_state, "presenter").ok_or_else(|| {
                    InstallError::new(
                        "existing profile requires a managed presenter, but matching installation state is unavailable",
                    )
                })?,
                true,
                ResolutionOrigin::Managed,
            ),
        };

        match self.resolver.resolve(&candidate) {
            Ok(resolved) => Ok(PresenterPlan::resolved(
                candidate, resolved, origin, selection,
            )),
            Err(error) if must_resolve => Err(InstallError::new(format!(
                "cannot select Chafa presenter from `{}`: {error}",
                candidate.display()
            ))),
            Err(error) => {
                warnings.push(format!(
                    "external engines were not activated because the presenter could not be resolved: {error}"
                ));
                Ok(PresenterPlan::missing(
                    candidate,
                    error.to_string(),
                    selection,
                ))
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
struct SlotPlan {
    spec: SlotSpec,
    route: SlotRoute,
    requested: Option<PathBuf>,
    resolved: Option<PathBuf>,
    required: bool,
    origin: ResolutionOrigin,
    note: Option<String>,
    selection: EngineSelection,
}

impl SlotPlan {
    fn builtin(
        spec: SlotSpec,
        route: SlotRoute,
        origin: ResolutionOrigin,
        selection: EngineSelection,
    ) -> Self {
        Self {
            spec,
            route,
            requested: None,
            resolved: None,
            required: false,
            origin,
            note: None,
            selection,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn external(
        spec: SlotSpec,
        requested: PathBuf,
        resolved: PathBuf,
        required: bool,
        origin: ResolutionOrigin,
        selection: EngineSelection,
    ) -> Self {
        Self {
            spec,
            route: SlotRoute::External,
            requested: Some(requested),
            resolved: Some(resolved),
            required,
            origin,
            note: None,
            selection,
        }
    }

    fn fallback(
        spec: SlotSpec,
        requested: PathBuf,
        reason: String,
        selection: EngineSelection,
    ) -> Self {
        Self {
            spec,
            route: SlotRoute::Preview,
            requested: Some(requested),
            resolved: None,
            required: false,
            origin: ResolutionOrigin::AutoFallback,
            note: Some(reason),
            selection,
        }
    }

    fn fallback_if_optional(&mut self, reason: &str, warnings: &mut Vec<String>) {
        if self.route == SlotRoute::External && !self.required {
            warnings.push(format!(
                "{} external engine was resolved but not activated: {reason}; using preview",
                self.spec.role
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
            SlotRoute::External => self.spec.external_backend,
        }
    }

    fn into_component(self) -> InstalledComponent {
        InstalledComponent {
            role: self.spec.role.to_owned(),
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
    selection: PresenterSelection,
}

impl PresenterPlan {
    fn resolved(
        requested: PathBuf,
        resolved: PathBuf,
        origin: ResolutionOrigin,
        selection: PresenterSelection,
    ) -> Self {
        Self {
            requested,
            resolved: Some(resolved),
            active: true,
            origin,
            note: None,
            selection,
        }
    }

    fn inactive(
        requested: PathBuf,
        origin: ResolutionOrigin,
        selection: PresenterSelection,
    ) -> Self {
        Self {
            requested,
            resolved: None,
            active: false,
            origin,
            note: None,
            selection,
        }
    }

    fn missing(requested: PathBuf, reason: String, selection: PresenterSelection) -> Self {
        Self {
            requested,
            resolved: None,
            active: false,
            origin: ResolutionOrigin::AutoFallback,
            note: Some(reason),
            selection,
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

pub fn default_install_state_path() -> Result<PathBuf, InstallError> {
    PlatformPaths::discover()
        .map(|paths| paths.install_state_file)
        .map_err(InstallError::new)
}

fn state_program(state: Option<&InstallState>, role: &str) -> Option<PathBuf> {
    state?
        .components
        .iter()
        .find(|component| component.role == role && component.active)
        .and_then(|component| component.resolved_path.clone())
}

fn absolute_path(path: &Path) -> Result<PathBuf, InstallError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    env::current_dir()
        .map(|directory| directory.join(path))
        .map_err(|error| InstallError::new(format!("cannot resolve current directory: {error}")))
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), InstallError> {
    let parent = path.parent().ok_or_else(|| {
        InstallError::new(format!("path `{}` has no parent directory", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        InstallError::new(format!(
            "cannot create installation directory `{}`: {error}",
            parent.display()
        ))
    })?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".ptymark-install-")
        .tempfile_in(parent)
        .map_err(|error| {
            InstallError::new(format!(
                "cannot create temporary installation file in `{}`: {error}",
                parent.display()
            ))
        })?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|error| InstallError::new(format!("cannot stage installation file: {error}")))?;
    temporary.persist(path).map_err(|error| {
        InstallError::new(format!(
            "cannot replace installation file `{}`: {}",
            path.display(),
            error.error
        ))
    })?;
    Ok(())
}

fn restore_previous(path: &Path, previous: Option<&[u8]>) -> Result<(), InstallError> {
    match previous {
        Some(bytes) => atomic_replace(path, bytes),
        None => {
            if path.exists() {
                fs::remove_file(path).map_err(|error| {
                    InstallError::new(format!(
                        "cannot remove newly written installation state `{}`: {error}",
                        path.display()
                    ))
                })?;
            }
            Ok(())
        }
    }
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
