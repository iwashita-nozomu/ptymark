use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderMode {
    #[default]
    Preview,
    Source,
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            detection: DetectionConfig::default(),
            rendering: RenderingConfig::default(),
            cache: CacheConfig::default(),
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
        Ok(())
    }

    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self)
            .map_err(|error| ConfigError::new(format!("cannot serialize configuration: {error}")))
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
    use super::{CONFIG_SCHEMA_VERSION, Config};

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
}
