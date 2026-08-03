use crate::config::{ConfigError, DetectionConfig, UserConfig};
use crate::limits::MAX_SEMANTIC_BLOCK_BYTES;
use sha2::{Digest, Sha256};

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            mermaid: true,
            math: true,
            max_block_bytes: MAX_SEMANTIC_BLOCK_BYTES,
        }
    }
}

impl UserConfig {
    /// SHA-256 identity of the normalized user-authored schema. Resolved
    /// executable paths and internal safety constants do not participate.
    pub fn fingerprint(&self) -> Result<String, ConfigError> {
        let normalized = self.to_toml()?;
        let digest = Sha256::digest(normalized.as_bytes());
        Ok(format!("{digest:x}"))
    }
}
