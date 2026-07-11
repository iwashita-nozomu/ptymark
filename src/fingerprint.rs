/// Produce a deterministic non-cryptographic fingerprint for in-process identity keys.
///
/// This uses FNV-1a so results are stable across runs and platforms. It is suitable for
/// cache/options identity and diagnostics correlation. It is not collision resistant and must not
/// be used for project trust, signatures, authentication, or persistent artifact integrity.
pub fn stable_fingerprint(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001b3;

    bytes.iter().fold(OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::stable_fingerprint;

    #[test]
    fn fingerprint_is_deterministic_and_order_sensitive() {
        assert_eq!(stable_fingerprint(b"ptymark"), stable_fingerprint(b"ptymark"));
        assert_ne!(stable_fingerprint(b"ptymark"), stable_fingerprint(b"ptymrak"));
    }
}
