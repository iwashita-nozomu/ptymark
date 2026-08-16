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
        Ok(encode_lower_hex(digest.as_ref()))
    }
}

fn encode_lower_hex(bytes: &[u8]) -> String {
    const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::encode_lower_hex;
    use sha2::{Digest, Sha256};

    #[test]
    fn encodes_sha256_digest_as_lowercase_hex() {
        let digest = Sha256::digest(b"abc");

        assert_eq!(
            encode_lower_hex(digest.as_ref()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
