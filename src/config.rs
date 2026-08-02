use crate::install::{InstallState, default_install_state_path};
use crate::limits::{
    MAX_FALLBACK_COLUMNS, MAX_SEMANTIC_BLOCK_BYTES, MAX_USER_CACHE_BYTES,
    MAX_USER_CACHE_ENTRIES,
};
use crate::managed_launcher::inspect_managed_alias;
use crate::platform::PlatformPaths;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_SCHEMA_VERSION: u32 = 2;
pub const LEGACY_CONFIG_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_PROFILE: &str = "default";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderMode {
    #[default]
    Preview,
    Source,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionMode {
    #[default]
    Render,
    Source,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PresentationMode {
    #[default]
    Auto,
    Symbols,
    Plain,
    Source,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorPolicy {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheBackend {
    None,
    #[default]
    Memory,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineProvider {
    #[default]
    Auto,
    Preview,
    Source,
    Managed,
    External,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PresenterProvider {
    #[default]
    Auto,
    Managed,
    External,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MermaidEngine {
    #[default]
    Preview,
    Source,
    MermaidCli,
}

impl MermaidEngine {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Source => "source",
            Self::MermaidCli => "mermaid-cli",
        }
    }

    pub const fn is_external(self) -> bool {
        matches!(self, Self::MermaidCli)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MathEngine {
    #[default]
    Preview,
    Source,
    MathjaxCli,
}

impl MathEngine {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Source => "source",
            Self::MathjaxCli => "mathjax-cli",
        }
    }

    pub const fn is_external(self) -> bool {
        matches!(self, Self::MathjaxCli)
    }
}

/// User-authored schema. This contains stable intent only: profiles, selected
/// providers, presentation preferences, and bounded local cache preferences.
/// Resolved executable paths, installation ownership, temporary locations, and
/// hard safety limits are deliberately absent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserConfig {
    pub schema_version: u32,
    #[serde(default = "default_profile_name")]
    pub default_profile: String,
    #[serde(default = "default_profiles")]
    pub profiles: BTreeMap<String, ProfileConfig>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            default_profile: default_profile_name(),
            profiles: default_profiles(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProfileConfig {
    pub session: SessionConfig,
    pub detection: UserDetectionConfig,
    pub presentation: UserPresentationConfig,
    pub cache: UserCacheConfig,
    pub engines: UserEnginesConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SessionConfig {
    pub mode: SessionMode,
    pub strict: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            mode: SessionMode::Render,
            strict: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UserDetectionConfig {
    pub mermaid: bool,
    pub math: bool,
}

impl Default for UserDetectionConfig {
    fn default() -> Self {
        Self {
            mermaid: true,
            math: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UserPresentationConfig {
    pub mode: PresentationMode,
    pub color: ColorPolicy,
    pub fallback_columns: u16,
}

impl Default for UserPresentationConfig {
    fn default() -> Self {
        Self {
            mode: PresentationMode::Auto,
            color: ColorPolicy::Auto,
            fallback_columns: 80,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UserCacheConfig {
    pub backend: CacheBackend,
    pub max_entries: usize,
    pub max_bytes: usize,
}

impl Default for UserCacheConfig {
    fn default() -> Self {
        Self {
            backend: CacheBackend::Memory,
            max_entries: 128,
            max_bytes: 32 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UserEnginesConfig {
    pub mermaid: EngineSelection,
    pub math: EngineSelection,
    pub presenter: PresenterSelection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EngineSelection {
    pub provider: EngineProvider,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<PathBuf>,
}

impl Default for EngineSelection {
    fn default() -> Self {
        Self {
            provider: EngineProvider::Auto,
            program: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PresenterSelection {
    pub provider: PresenterProvider,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<PathBuf>,
}

impl Default for PresenterSelection {
    fn default() -> Self {
        Self {
            provider: PresenterProvider::Auto,
            program: None,
        }
    }
}

/// Resolved, immutable session configuration. This type is internal runtime
/// state even though it remains public during the alpha compatibility period.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub schema_version: u32,
    pub selected_profile: String,
    pub detection: DetectionConfig,
    pub rendering: RenderingConfig,
    pub cache: CacheConfig,
    pub engines: EnginesConfig,
    user: UserConfig,
}

impl Default for Config {
    fn default() -> Self {
        UserConfig::default()
            .resolve(None, None)
            .expect("the built-in user configuration must resolve")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionConfig {
    pub mermaid: bool,
    pub math: bool,
    pub max_block_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderingConfig {
    pub mode: RenderMode,
    pub strict: bool,
    pub columns: u16,
    pub presentation: PresentationMode,
    pub color: ColorPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheConfig {
    pub enabled: bool,
    pub max_entries: usize,
    pub max_bytes: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EnginesConfig {
    pub mermaid: MermaidEngineConfig,
    pub math: MathEngineConfig,
    pub presenter: PresenterConfig,
}

impl EnginesConfig {
    pub const fn uses_external_engine(&self) -> bool {
        self.mermaid.backend.is_external() || self.math.backend.is_external()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MermaidEngineConfig {
    pub backend: MermaidEngine,
    pub path: PathBuf,
}

impl Default for MermaidEngineConfig {
    fn default() -> Self {
        Self {
            backend: MermaidEngine::Preview,
            path: PathBuf::from("mmdc"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathEngineConfig {
    pub backend: MathEngine,
    pub path: PathBuf,
}

impl Default for MathEngineConfig {
    fn default() -> Self {
        Self {
            backend: MathEngine::Preview,
            path: PathBuf::from("tex2svg"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresenterConfig {
    pub path: PathBuf,
}

impl Default for PresenterConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("chafa"),
        }
    }
}

impl UserConfig {
    pub fn load_exact(path: &Path) -> Result<Self, ConfigError> {
        let source = fs::read_to_string(path).map_err(|error| {
            ConfigError::new(format!("cannot read `{}`: {error}", path.display()))
        })?;
        Self::parse(&source, path)
    }

    pub fn parse(source: &str, path: &Path) -> Result<Self, ConfigError> {
        let value: toml::Value = toml::from_str(source).map_err(|error| {
            ConfigError::new(format!("cannot parse `{}`: {error}", path.display()))
        })?;
        let schema = value
            .get("schema_version")
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "configuration `{}` requires an integer schema_version",
                    path.display()
                ))
            })?;
        let config = match u32::try_from(schema) {
            Ok(CONFIG_SCHEMA_VERSION) => toml::from_str::<Self>(source).map_err(|error| {
                ConfigError::new(format!("cannot parse `{}`: {error}", path.display()))
            })?,
            Ok(LEGACY_CONFIG_SCHEMA_VERSION) => {
                let legacy: LegacyConfig = toml::from_str(source).map_err(|error| {
                    ConfigError::new(format!(
                        "cannot parse legacy configuration `{}`: {error}",
                        path.display()
                    ))
                })?;
                legacy.into_user_config()
            }
            Ok(version) => {
                return Err(ConfigError::new(format!(
                    "unsupported schema_version {version}; supported versions are {LEGACY_CONFIG_SCHEMA_VERSION} and {CONFIG_SCHEMA_VERSION}"
                )));
            }
            Err(_) => {
                return Err(ConfigError::new(format!(
                    "schema_version {schema} is outside the supported integer range"
                )));
            }
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != CONFIG_SCHEMA_VERSION {
            return Err(ConfigError::new(format!(
                "normalized user configuration must use schema_version {CONFIG_SCHEMA_VERSION}"
            )));
        }
        if self.default_profile.trim().is_empty() {
            return Err(ConfigError::new("default_profile cannot be empty"));
        }
        if !self.profiles.contains_key(&self.default_profile) {
            return Err(ConfigError::new(format!(
                "default_profile `{}` is not defined under profiles",
                self.default_profile
            )));
        }
        for (name, profile) in &self.profiles {
            if name.trim().is_empty() || name.chars().any(char::is_control) {
                return Err(ConfigError::new(
                    "profile names must be non-empty and contain no control characters",
                ));
            }
            profile.validate(name)?;
        }
        Ok(())
    }

    pub fn resolve(
        &self,
        selected_profile: Option<&str>,
        state: Option<&InstallState>,
    ) -> Result<Config, ConfigError> {
        self.validate()?;
        let selected_profile = selected_profile.unwrap_or(&self.default_profile);
        let profile = self.profiles.get(selected_profile).ok_or_else(|| {
            ConfigError::new(format!("profile `{selected_profile}` is not defined"))
        })?;
        let source_requested = profile.session.mode == SessionMode::Source
            || profile.presentation.mode == PresentationMode::Source;
        let plain_requested = profile.presentation.mode == PresentationMode::Plain;

        let mermaid = resolve_mermaid(
            &profile.engines.mermaid,
            state,
            source_requested,
            plain_requested,
        )?;
        let math = resolve_math(
            &profile.engines.math,
            state,
            source_requested,
            plain_requested,
        )?;
        let presenter = resolve_presenter(
            &profile.engines.presenter,
            state,
            mermaid.backend.is_external() || math.backend.is_external(),
        )?;

        Ok(Config {
            schema_version: CONFIG_SCHEMA_VERSION,
            selected_profile: selected_profile.to_owned(),
            detection: DetectionConfig {
                mermaid: profile.detection.mermaid,
                math: profile.detection.math,
                max_block_bytes: MAX_SEMANTIC_BLOCK_BYTES,
            },
            rendering: RenderingConfig {
                mode: if source_requested {
                    RenderMode::Source
                } else {
                    RenderMode::Preview
                },
                strict: profile.session.strict,
                columns: profile.presentation.fallback_columns,
                presentation: profile.presentation.mode,
                color: profile.presentation.color,
            },
            cache: CacheConfig {
                enabled: profile.cache.backend == CacheBackend::Memory,
                max_entries: profile.cache.max_entries,
                max_bytes: profile.cache.max_bytes,
            },
            engines: EnginesConfig {
                mermaid,
                math,
                presenter,
            },
            user: self.clone(),
        })
    }

    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self)
            .map_err(|error| ConfigError::new(format!("cannot serialize configuration: {error}")))
    }

    pub fn profile_mut(&mut self, name: &str) -> Result<&mut ProfileConfig, ConfigError> {
        self.profiles
            .get_mut(name)
            .ok_or_else(|| ConfigError::new(format!("profile `{name}` is not defined")))
    }
}

impl ProfileConfig {
    fn validate(&self, name: &str) -> Result<(), ConfigError> {
        if self.presentation.fallback_columns == 0
            || self.presentation.fallback_columns > MAX_FALLBACK_COLUMNS
        {
            return Err(ConfigError::new(format!(
                "profiles.{name}.presentation.fallback_columns must be between 1 and {MAX_FALLBACK_COLUMNS}"
            )));
        }
        if self.cache.max_entries > MAX_USER_CACHE_ENTRIES {
            return Err(ConfigError::new(format!(
                "profiles.{name}.cache.max_entries exceeds the supported preference bound {MAX_USER_CACHE_ENTRIES}"
            )));
        }
        if self.cache.max_bytes > MAX_USER_CACHE_BYTES {
            return Err(ConfigError::new(format!(
                "profiles.{name}.cache.max_bytes exceeds the supported preference bound {MAX_USER_CACHE_BYTES}"
            )));
        }
        if self.cache.backend == CacheBackend::Memory
            && (self.cache.max_entries == 0 || self.cache.max_bytes == 0)
        {
            return Err(ConfigError::new(format!(
                "profiles.{name}.cache memory backend requires positive max_entries and max_bytes"
            )));
        }
        validate_engine_selection(
            &format!("profiles.{name}.engines.mermaid"),
            &self.engines.mermaid,
        )?;
        validate_engine_selection(
            &format!("profiles.{name}.engines.math"),
            &self.engines.math,
        )?;
        validate_presenter_selection(
            &format!("profiles.{name}.engines.presenter"),
            &self.engines.presenter,
        )?;
        Ok(())
    }
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        Self::load_profile(path, None)
    }

    pub fn load_profile(
        path: Option<&Path>,
        selected_profile: Option<&str>,
    ) -> Result<Self, ConfigError> {
        match path {
            Some(path) => Self::load_exact_profile(path, selected_profile),
            None => {
                let path = Self::user_config_path()?;
                if path.is_file() {
                    Self::load_exact_profile(&path, selected_profile)
                } else {
                    UserConfig::default().resolve(selected_profile, None)
                }
            }
        }
    }

    pub fn load_exact(path: &Path) -> Result<Self, ConfigError> {
        Self::load_exact_profile(path, None)
    }

    pub fn load_exact_profile(
        path: &Path,
        selected_profile: Option<&str>,
    ) -> Result<Self, ConfigError> {
        let user = UserConfig::load_exact(path)?;
        let state = load_matching_install_state(path);
        user.resolve(selected_profile, state.as_ref())
    }

    pub fn user_config_path() -> Result<PathBuf, ConfigError> {
        PlatformPaths::discover()
            .map(|paths| paths.config_file)
            .map_err(ConfigError::new)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        self.user.validate()?;
        if self.schema_version != CONFIG_SCHEMA_VERSION {
            return Err(ConfigError::new(format!(
                "resolved configuration schema {} does not match {CONFIG_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        Ok(())
    }

    /// Return normalized user-authored TOML. Resolved executable paths and hard
    /// internal limits are intentionally not serialized by this method.
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        self.user.to_toml()
    }

    pub fn user_config(&self) -> &UserConfig {
        &self.user
    }
}

fn load_matching_install_state(config_path: &Path) -> Option<InstallState> {
    let state_path = default_install_state_path().ok()?;
    let state = InstallState::load(&state_path).ok()?;
    (state.config_path == config_path).then_some(state)
}

fn resolve_mermaid(
    selection: &EngineSelection,
    state: Option<&InstallState>,
    source_requested: bool,
    plain_requested: bool,
) -> Result<MermaidEngineConfig, ConfigError> {
    if source_requested {
        return Ok(MermaidEngineConfig {
            backend: MermaidEngine::Source,
            path: PathBuf::from("mmdc"),
        });
    }
    if plain_requested {
        return Ok(MermaidEngineConfig::default());
    }
    match selection.provider {
        EngineProvider::Preview => Ok(MermaidEngineConfig::default()),
        EngineProvider::Source => Ok(MermaidEngineConfig {
            backend: MermaidEngine::Source,
            path: PathBuf::from("mmdc"),
        }),
        EngineProvider::External => Ok(MermaidEngineConfig {
            backend: MermaidEngine::MermaidCli,
            path: selection
                .program
                .clone()
                .expect("validated external program"),
        }),
        EngineProvider::Managed => Ok(MermaidEngineConfig {
            backend: MermaidEngine::MermaidCli,
            path: managed_state_program(state, "mermaid")?,
        }),
        EngineProvider::Auto => Ok(state_program(state, "mermaid").map_or_else(
            MermaidEngineConfig::default,
            |path| MermaidEngineConfig {
                backend: MermaidEngine::MermaidCli,
                path,
            },
        )),
    }
}

fn resolve_math(
    selection: &EngineSelection,
    state: Option<&InstallState>,
    source_requested: bool,
    plain_requested: bool,
) -> Result<MathEngineConfig, ConfigError> {
    if source_requested {
        return Ok(MathEngineConfig {
            backend: MathEngine::Source,
            path: PathBuf::from("tex2svg"),
        });
    }
    if plain_requested {
        return Ok(MathEngineConfig::default());
    }
    match selection.provider {
        EngineProvider::Preview => Ok(MathEngineConfig::default()),
        EngineProvider::Source => Ok(MathEngineConfig {
            backend: MathEngine::Source,
            path: PathBuf::from("tex2svg"),
        }),
        EngineProvider::External => Ok(MathEngineConfig {
            backend: MathEngine::MathjaxCli,
            path: selection
                .program
                .clone()
                .expect("validated external program"),
        }),
        EngineProvider::Managed => Ok(MathEngineConfig {
            backend: MathEngine::MathjaxCli,
            path: managed_state_program(state, "math")?,
        }),
        EngineProvider::Auto => Ok(state_program(state, "math").map_or_else(
            MathEngineConfig::default,
            |path| MathEngineConfig {
                backend: MathEngine::MathjaxCli,
                path,
            },
        )),
    }
}

fn resolve_presenter(
    selection: &PresenterSelection,
    state: Option<&InstallState>,
    required: bool,
) -> Result<PresenterConfig, ConfigError> {
    if !required {
        return Ok(PresenterConfig::default());
    }
    match selection.provider {
        PresenterProvider::External => Ok(PresenterConfig {
            path: selection
                .program
                .clone()
                .expect("validated external presenter program"),
        }),
        PresenterProvider::Managed => Ok(PresenterConfig {
            path: managed_state_program(state, "presenter")?,
        }),
        PresenterProvider::Auto => Ok(PresenterConfig {
            path: state_program(state, "presenter").unwrap_or_else(|| PathBuf::from("chafa")),
        }),
    }
}

fn state_program(state: Option<&InstallState>, role: &str) -> Option<PathBuf> {
    state?
        .components
        .iter()
        .find(|component| component.role == role && component.active)
        .and_then(|component| component.resolved_path.clone())
}

fn managed_state_program(
    state: Option<&InstallState>,
    role: &str,
) -> Result<PathBuf, ConfigError> {
    let path = state_program(state, role).ok_or_else(|| {
        ConfigError::new(format!(
            "profile requires the managed {role} role, but no matching installation state is available"
        ))
    })?;
    if inspect_managed_alias(&path).is_none() {
        return Err(ConfigError::new(format!(
            "profile requires the managed {role} role, but `{}` is not a managed alias",
            path.display()
        )));
    }
    Ok(path)
}

fn validate_engine_selection(label: &str, selection: &EngineSelection) -> Result<(), ConfigError> {
    match (selection.provider, selection.program.as_deref()) {
        (EngineProvider::External, Some(path)) => validate_program_path(&format!("{label}.program"), path),
        (EngineProvider::External, None) => Err(ConfigError::new(format!(
            "{label}.program is required when provider is external"
        ))),
        (_, Some(_)) => Err(ConfigError::new(format!(
            "{label}.program is only valid when provider is external"
        ))),
        (_, None) => Ok(()),
    }
}

fn validate_presenter_selection(
    label: &str,
    selection: &PresenterSelection,
) -> Result<(), ConfigError> {
    match (selection.provider, selection.program.as_deref()) {
        (PresenterProvider::External, Some(path)) => {
            validate_program_path(&format!("{label}.program"), path)
        }
        (PresenterProvider::External, None) => Err(ConfigError::new(format!(
            "{label}.program is required when provider is external"
        ))),
        (_, Some(_)) => Err(ConfigError::new(format!(
            "{label}.program is only valid when provider is external"
        ))),
        (_, None) => Ok(()),
    }
}

fn validate_program_path(label: &str, path: &Path) -> Result<(), ConfigError> {
    if path.as_os_str().is_empty() {
        return Err(ConfigError::new(format!("{label} cannot be empty")));
    }
    if !path.is_absolute() && path.components().count() != 1 {
        return Err(ConfigError::new(format!(
            "{label} must be an absolute path or a bare executable name"
        )));
    }
    Ok(())
}

fn default_profile_name() -> String {
    DEFAULT_PROFILE.to_owned()
}

fn default_profiles() -> BTreeMap<String, ProfileConfig> {
    BTreeMap::from([(DEFAULT_PROFILE.to_owned(), ProfileConfig::default())])
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyConfig {
    schema_version: u32,
    #[serde(default)]
    detection: LegacyDetectionConfig,
    #[serde(default)]
    rendering: LegacyRenderingConfig,
    #[serde(default)]
    cache: LegacyCacheConfig,
    #[serde(default)]
    engines: LegacyEnginesConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LegacyDetectionConfig {
    mermaid: bool,
    math: bool,
    max_block_bytes: usize,
}

impl Default for LegacyDetectionConfig {
    fn default() -> Self {
        Self {
            mermaid: true,
            math: true,
            max_block_bytes: MAX_SEMANTIC_BLOCK_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum LegacyRenderMode {
    #[default]
    Preview,
    Source,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LegacyRenderingConfig {
    mode: LegacyRenderMode,
    strict: bool,
    columns: u16,
}

impl Default for LegacyRenderingConfig {
    fn default() -> Self {
        Self {
            mode: LegacyRenderMode::Preview,
            strict: false,
            columns: 80,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LegacyCacheConfig {
    enabled: bool,
    max_entries: usize,
    max_bytes: usize,
}

impl Default for LegacyCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 128,
            max_bytes: 32 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum LegacyMermaidEngine {
    #[default]
    Preview,
    Source,
    MermaidCli,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum LegacyMathEngine {
    #[default]
    Preview,
    Source,
    MathjaxCli,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LegacyEnginesConfig {
    mermaid: LegacyMermaidEngineConfig,
    math: LegacyMathEngineConfig,
    presenter: LegacyPresenterConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LegacyMermaidEngineConfig {
    backend: LegacyMermaidEngine,
    path: PathBuf,
}

impl Default for LegacyMermaidEngineConfig {
    fn default() -> Self {
        Self {
            backend: LegacyMermaidEngine::Preview,
            path: PathBuf::from("mmdc"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LegacyMathEngineConfig {
    backend: LegacyMathEngine,
    path: PathBuf,
}

impl Default for LegacyMathEngineConfig {
    fn default() -> Self {
        Self {
            backend: LegacyMathEngine::Preview,
            path: PathBuf::from("tex2svg"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LegacyPresenterConfig {
    path: PathBuf,
}

impl Default for LegacyPresenterConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("chafa"),
        }
    }
}

impl LegacyConfig {
    fn into_user_config(self) -> UserConfig {
        debug_assert_eq!(self.schema_version, LEGACY_CONFIG_SCHEMA_VERSION);
        let mermaid = match self.engines.mermaid.backend {
            LegacyMermaidEngine::Preview => EngineSelection {
                provider: EngineProvider::Preview,
                program: None,
            },
            LegacyMermaidEngine::Source => EngineSelection {
                provider: EngineProvider::Source,
                program: None,
            },
            LegacyMermaidEngine::MermaidCli => legacy_external_selection(self.engines.mermaid.path),
        };
        let math = match self.engines.math.backend {
            LegacyMathEngine::Preview => EngineSelection {
                provider: EngineProvider::Preview,
                program: None,
            },
            LegacyMathEngine::Source => EngineSelection {
                provider: EngineProvider::Source,
                program: None,
            },
            LegacyMathEngine::MathjaxCli => legacy_external_selection(self.engines.math.path),
        };
        let external_selected = matches!(mermaid.provider, EngineProvider::External | EngineProvider::Managed)
            || matches!(math.provider, EngineProvider::External | EngineProvider::Managed);
        let presenter = if external_selected {
            if inspect_managed_alias(&self.engines.presenter.path).is_some() {
                PresenterSelection {
                    provider: PresenterProvider::Managed,
                    program: None,
                }
            } else {
                PresenterSelection {
                    provider: PresenterProvider::External,
                    program: Some(self.engines.presenter.path),
                }
            }
        } else {
            PresenterSelection::default()
        };
        let profile = ProfileConfig {
            session: SessionConfig {
                mode: if self.rendering.mode == LegacyRenderMode::Source {
                    SessionMode::Source
                } else {
                    SessionMode::Render
                },
                strict: self.rendering.strict,
            },
            detection: UserDetectionConfig {
                mermaid: self.detection.mermaid,
                math: self.detection.math,
            },
            presentation: UserPresentationConfig {
                mode: if self.rendering.mode == LegacyRenderMode::Source {
                    PresentationMode::Source
                } else {
                    PresentationMode::Auto
                },
                color: ColorPolicy::Auto,
                fallback_columns: self.rendering.columns.max(1).min(MAX_FALLBACK_COLUMNS),
            },
            cache: UserCacheConfig {
                backend: if self.cache.enabled {
                    CacheBackend::Memory
                } else {
                    CacheBackend::None
                },
                max_entries: self.cache.max_entries.min(MAX_USER_CACHE_ENTRIES),
                max_bytes: self.cache.max_bytes.min(MAX_USER_CACHE_BYTES),
            },
            engines: UserEnginesConfig {
                mermaid,
                math,
                presenter,
            },
        };
        UserConfig {
            schema_version: CONFIG_SCHEMA_VERSION,
            default_profile: default_profile_name(),
            profiles: BTreeMap::from([(DEFAULT_PROFILE.to_owned(), profile)]),
        }
    }
}

fn legacy_external_selection(path: PathBuf) -> EngineSelection {
    if inspect_managed_alias(&path).is_some() {
        EngineSelection {
            provider: EngineProvider::Managed,
            program: None,
        }
    } else {
        EngineSelection {
            provider: EngineProvider::External,
            program: Some(path),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::{
        CONFIG_SCHEMA_VERSION, Config, EngineProvider, PresentationMode, UserConfig,
    };
    use std::fs;

    #[test]
    fn defaults_are_valid_and_use_schema_v2() {
        let config = Config::default();
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(config.selected_profile, "default");
        config.validate().expect("default config");
    }

    #[test]
    fn normalized_user_config_does_not_serialize_internal_limits_or_paths() {
        let source = UserConfig::default().to_toml().expect("serialize");
        assert!(source.contains("schema_version = 2"));
        assert!(source.contains("[profiles.default.presentation]"));
        assert!(!source.contains("max_block_bytes"));
        assert!(!source.contains("render_timeout"));
        assert!(!source.contains("resolved_path"));
    }

    #[test]
    fn unknown_v2_keys_are_rejected() {
        let root = tempfile::tempdir().expect("temp root");
        let path = root.path().join("config.toml");
        fs::write(
            &path,
            "schema_version = 2\ndefault_profile = 'default'\nunknown = true\n",
        )
        .expect("write");
        let error = UserConfig::load_exact(&path).expect_err("unknown keys must fail");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn legacy_v1_is_migrated_to_the_profile_model() {
        let root = tempfile::tempdir().expect("temp root");
        let path = root.path().join("legacy.toml");
        fs::write(
            &path,
            "schema_version = 1\n\n[rendering]\nmode = 'source'\nstrict = true\ncolumns = 100\n\n[cache]\nenabled = false\n",
        )
        .expect("write");
        let user = UserConfig::load_exact(&path).expect("migrate");
        assert_eq!(user.schema_version, 2);
        let profile = &user.profiles["default"];
        assert_eq!(profile.presentation.mode, PresentationMode::Source);
        assert!(profile.session.strict);
        assert_eq!(profile.engines.mermaid.provider, EngineProvider::Preview);
        assert!(!user.to_toml().expect("normalized").contains("max_block_bytes"));
    }

    #[test]
    fn external_provider_requires_a_program() {
        let mut user = UserConfig::default();
        user.profiles
            .get_mut("default")
            .expect("profile")
            .engines
            .math
            .provider = EngineProvider::External;
        let error = user.validate().expect_err("missing program");
        assert!(error.to_string().contains("program is required"));
    }

    #[test]
    fn exact_file_round_trip_preserves_user_intent() {
        let root = tempfile::tempdir().expect("temp root");
        let path = root.path().join("config.toml");
        let user = UserConfig::default();
        fs::write(&path, user.to_toml().expect("serialize")).expect("write");
        assert_eq!(UserConfig::load_exact(&path).expect("load"), user);
    }
}
