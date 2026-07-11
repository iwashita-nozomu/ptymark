use super::{ConfigError, ConfigProvenance, LoadedConfig, ResolvedConfig};
use crate::fingerprint::stable_fingerprint;
use std::sync::Arc;

/// Immutable configuration identity for one preview or terminal session.
///
/// Session overrides are applied before this value is created. Clones share the exact same
/// resolved policy and provenance through `Arc`; a reload must create a new generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSnapshot {
    generation: u64,
    fingerprint: u64,
    config: Arc<ResolvedConfig>,
    provenance: Arc<ConfigProvenance>,
}

impl ConfigSnapshot {
    pub fn new(
        generation: u64,
        config: ResolvedConfig,
        provenance: ConfigProvenance,
    ) -> Result<Self, ConfigError> {
        let material = toml::to_string(&config)
            .map(String::into_bytes)
            .map_err(|error| {
                ConfigError::validation(format!("cannot serialize config identity: {error}"))
            })?;
        Ok(Self {
            generation,
            fingerprint: stable_fingerprint(&material),
            config: Arc::new(config),
            provenance: Arc::new(provenance),
        })
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub fn config(&self) -> &ResolvedConfig {
        &self.config
    }

    pub fn provenance(&self) -> &ConfigProvenance {
        &self.provenance
    }

    pub fn config_arc(&self) -> Arc<ResolvedConfig> {
        Arc::clone(&self.config)
    }

    pub fn provenance_arc(&self) -> Arc<ConfigProvenance> {
        Arc::clone(&self.provenance)
    }
}

impl LoadedConfig {
    pub fn into_snapshot(self, generation: u64) -> Result<ConfigSnapshot, ConfigError> {
        ConfigSnapshot::new(generation, self.config, self.provenance)
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigSnapshot;
    use crate::config::{ConfigEnvironment, ConfigManager, ConfigRequest};

    fn snapshot(generation: u64) -> ConfigSnapshot {
        ConfigManager::default()
            .load(ConfigRequest::default(), ConfigEnvironment::default())
            .expect("load built-in configuration")
            .into_snapshot(generation)
            .expect("freeze snapshot")
    }

    #[test]
    fn equal_policy_has_stable_fingerprint_across_generations() {
        let first = snapshot(1);
        let second = snapshot(2);
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_ne!(first.generation(), second.generation());
    }

    #[test]
    fn clones_share_the_same_resolved_policy() {
        let first = snapshot(7);
        let second = first.clone();
        assert_eq!(first, second);
        assert!(std::sync::Arc::ptr_eq(
            &first.config_arc(),
            &second.config_arc()
        ));
    }
}
