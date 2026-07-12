use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderMode {
    #[default]
    Preview,
    Source,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    #[serde(default)]
    pub detection: DetectionConfig,
    #[serde(default)]
    pub rendering: RenderingConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub engines: EnginesConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            detection: DetectionConfig::default(),
            rendering: RenderingConfig::default(),
            cache: CacheConfig::default(),
            engines: EnginesConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DetectionConfig {
    pub mermaid: bool,
    pub math: bool,
    pub max_block_bytes: usize,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            mermaid: true,
            math: true,
            max_block_bytes: 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RenderingConfig {
    pub mode: RenderMode,
    pub strict: bool,
    pub columns: u16,
}

impl Default for RenderingConfig {
    fn default() -> Self {
        Self {
            mode: RenderMode::Preview,
            strict: false,
            columns: 80,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CacheConfig {
    pub enabled: bool,
    pub max_entries: usize,
    pub max_bytes: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 128,
            max_bytes: 32 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnginesConfig {
    pub mermaid: MermaidEngineConfig,
    pub math: MathEngineConfig,
    pub presenter: PresenterConfig,
}

impl Default for EnginesConfig {
    fn default() -> Self {
        Self {
            mermaid: MermaidEngineConfig::default(),
            math: MathEngineConfig::default(),
            presenter: PresenterConfig::default(),
        }
    }
}

impl EnginesConfig {
    pub const fn uses_external_engine(&self) -> bool {
        self.mermaid.backend.is_external() || self.math.backend.is_external()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
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

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let config = match path {
            Some(path) => {
                let source = fs::read_to_string(path).map_err(|error| {
                    ConfigError::new(format!("cannot read `{}`: {error}", path.display()))
                })?;
                toml::from_str(&source).map_err(|error| {
                    ConfigError::new(format!("cannot parse `{}`: {error}", path.display()))
                })?
            }
            None => Self::default(),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != CONFIG_SCHEMA_VERSION {
            return Err(ConfigError::new(format!(
                "unsupported schema_version {}; expected {}",
                self.schema_version, CONFIG_SCHEMA_VERSION
            )));
        }
        if self.detection.max_block_bytes == 0 {
            return Err(ConfigError::new(
                "detection.max_block_bytes must be greater than zero",
            ));
        }
        if self.rendering.columns == 0 {
            return Err(ConfigError::new(
                "rendering.columns must be greater than zero",
            ));
        }
        if self.cache.enabled && (self.cache.max_entries == 0 || self.cache.max_bytes == 0) {
            return Err(ConfigError::new(
                "enabled cache requires positive max_entries and max_bytes",
            ));
        }
        validate_program_path("engines.mermaid.path", &self.engines.mermaid.path)?;
        validate_program_path("engines.math.path", &self.engines.math.path)?;
        validate_program_path("engines.presenter.path", &self.engines.presenter.path)?;
        Ok(())
    }

    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self)
            .map_err(|error| ConfigError::new(format!("cannot serialize configuration: {error}")))
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
    use super::{CONFIG_SCHEMA_VERSION, Config, MermaidEngine};
    use std::path::PathBuf;

    #[test]
    fn defaults_are_valid() {
        let config = Config::default();
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        config.validate().expect("default config");
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let error = toml::from_str::<Config>("schema_version = 1\nunknown = true\n")
            .expect_err("unknown keys must fail");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn enabled_cache_requires_limits() {
        let mut config = Config::default();
        config.cache.max_entries = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn engine_paths_accept_absolute_paths_and_bare_names() {
        let mut config = Config::default();
        config.engines.mermaid.backend = MermaidEngine::MermaidCli;
        config.engines.mermaid.path = PathBuf::from("mmdc");
        config.validate().expect("bare executable name");

        config.engines.mermaid.path = PathBuf::from("/opt/homebrew/bin/mmdc");
        config.validate().expect("absolute executable path");
    }

    #[test]
    fn relative_engine_paths_with_directories_are_rejected() {
        let mut config = Config::default();
        config.engines.mermaid.path = PathBuf::from("tools/mmdc");
        let error = config.validate().expect_err("relative path must fail");
        assert!(error.to_string().contains("absolute path or a bare executable name"));
    }
}