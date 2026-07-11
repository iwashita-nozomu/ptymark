mod model;
mod resolve;
mod source;

pub use model::{
    CONFIG_SCHEMA_VERSION, CacheBackend, CacheConfig, CachePolicyConfig, ConfigFile,
    ConfiguredExecutionModel, ConfiguredLayout, DetectionConfig, DetectionMode, DetectionPolicy,
    DiagnosticFormat, DiagnosticLevel, DiagnosticSink, DiagnosticsConfig, DiagnosticsPolicy,
    EngineSelectionConfig, EngineSelectionPolicy, EngineType, ExternalEngineConfig, FallbackPolicy,
    FenceConfig, PresentationConfig, PresentationMode, PresentationPolicy, ProfileConfig,
    RenderConfig, RenderOrdering, RenderPolicy, RendererBundleConfig, ResolvedConfig,
    RuntimeConfig, SessionMode, UnsupportedPresentation,
};
pub use resolve::ConfigManager;
pub use source::{
    ConfigEnvironment, ConfigLocator, ConfigOrigin, ConfigRequest, ConfigSource, ConfigTrust,
    FilesystemConfigLocator,
};

use serde::Serialize;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigErrorKind {
    Discovery,
    Io,
    Parse,
    Schema,
    Profile,
    Validation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigError {
    kind: ConfigErrorKind,
    path: Option<PathBuf>,
    message: String,
}

impl ConfigError {
    pub fn new(kind: ConfigErrorKind, path: Option<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            kind,
            path,
            message: message.into(),
        }
    }

    pub fn discovery(message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Discovery, None, message)
    }

    pub fn io(path: Option<PathBuf>, message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Io, path, message)
    }

    pub fn parse(path: Option<PathBuf>, message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Parse, path, message)
    }

    pub fn schema(path: Option<PathBuf>, message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Schema, path, message)
    }

    pub fn profile(message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Profile, None, message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ConfigErrorKind::Validation, None, message)
    }

    pub const fn kind(&self) -> ConfigErrorKind {
        self.kind
    }

    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.path {
            write!(formatter, "{}: {}", path.display(), self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl Error for ConfigError {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConfigProvenance {
    pub selected_profile: String,
    pub sources: Vec<ConfigSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedConfig {
    pub config: ResolvedConfig,
    pub provenance: ConfigProvenance,
}

impl LoadedConfig {
    /// Serialize the effective configuration for human inspection.
    ///
    /// Explicit external-process environment values may contain credentials or private paths, so
    /// values are redacted while keys remain visible and explainable.
    pub fn effective_toml(&self) -> Result<String, ConfigError> {
        let mut redacted = self.config.clone();
        for engine in redacted.external_engines.values_mut() {
            for value in engine.environment.values_mut() {
                *value = "<redacted>".to_owned();
            }
        }
        toml::to_string_pretty(&redacted)
            .map_err(|error| ConfigError::validation(format!("cannot serialize config: {error}")))
    }

    /// Return full deterministic policy material for process-local cache/options identity.
    ///
    /// This value must never be printed or logged because it can include external-engine
    /// environment values. It exists separately from `effective_toml()` to avoid cache collisions
    /// between materially different rendering configurations.
    pub fn fingerprint_material(&self) -> Result<Vec<u8>, ConfigError> {
        toml::to_string(&self.config)
            .map(String::into_bytes)
            .map_err(|error| {
                ConfigError::validation(format!("cannot serialize config identity: {error}"))
            })
    }

    pub fn provenance_toml(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(&self.provenance).map_err(|error| {
            ConfigError::validation(format!("cannot serialize config provenance: {error}"))
        })
    }
}

impl ResolvedConfig {
    /// Apply the process-local private-session override without weakening terminal safety.
    ///
    /// The override affects only pre-display services. It never changes input, termios, signal
    /// forwarding, resize forwarding, child environment, or exit-status behavior.
    pub fn apply_private_override(&mut self) {
        self.cache.backend = CacheBackend::None;
        self.cache.private = true;
        self.diagnostics.sink = DiagnosticSink::Stderr;
        self.diagnostics.path = None;
        self.diagnostics.include_source = false;
        self.diagnostics.metrics = false;
    }
}
