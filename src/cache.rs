use crate::model::{BlockKind, SemanticFormat};
use lru::LruCache;
use std::num::NonZeroUsize;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CacheKey {
    renderer_id: String,
    kind: BlockKind,
    format: SemanticFormat,
    source: Vec<u8>,
    columns: u16,
    color: bool,
    plain: bool,
    theme_fingerprint: u64,
}

impl CacheKey {
    /// Construct a cache key for the default source format of the semantic
    /// kind. This preserves the original public constructor for callers that
    /// create TeX or Mermaid blocks directly.
    pub fn new(
        renderer_id: impl Into<String>,
        kind: BlockKind,
        source: &[u8],
        columns: u16,
        color: bool,
        theme_fingerprint: u64,
    ) -> Self {
        Self::new_with_format(
            renderer_id,
            kind,
            SemanticFormat::default_for(kind),
            source,
            columns,
            color,
            theme_fingerprint,
        )
    }

    pub fn new_with_format(
        renderer_id: impl Into<String>,
        kind: BlockKind,
        format: SemanticFormat,
        source: &[u8],
        columns: u16,
        color: bool,
        theme_fingerprint: u64,
    ) -> Self {
        Self::new_with_presentation(
            renderer_id,
            kind,
            format,
            source,
            columns,
            color,
            false,
            theme_fingerprint,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_presentation(
        renderer_id: impl Into<String>,
        kind: BlockKind,
        format: SemanticFormat,
        source: &[u8],
        columns: u16,
        color: bool,
        plain: bool,
        theme_fingerprint: u64,
    ) -> Self {
        Self {
            renderer_id: renderer_id.into(),
            kind,
            format,
            source: source.to_vec(),
            columns,
            color,
            plain,
            theme_fingerprint,
        }
    }

    /// Logical bytes owned by one cache entry. Container allocator overhead is
    /// intentionally excluded; unlike the alpha.3 implementation, the key is
    /// stored exactly once and is not cloned into a second recency queue.
    fn weight(&self) -> usize {
        self.renderer_id
            .len()
            .saturating_add(self.source.len())
            .saturating_add(std::mem::size_of::<BlockKind>())
            .saturating_add(std::mem::size_of::<SemanticFormat>())
            .saturating_add(std::mem::size_of::<u16>())
            .saturating_add(std::mem::size_of::<bool>() * 2)
            .saturating_add(std::mem::size_of::<u64>())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub insertions: u64,
    pub evictions: u64,
    pub entries: usize,
    pub bytes: usize,
}

pub trait ArtifactCache: Send {
    fn get(&mut self, key: &CacheKey) -> Option<Vec<u8>>;
    fn put(&mut self, key: CacheKey, bytes: Vec<u8>) -> bool;
    fn clear(&mut self);
    fn stats(&self) -> CacheStats;
}

#[derive(Debug, Default)]
pub struct NoopCache {
    stats: CacheStats,
}

impl ArtifactCache for NoopCache {
    fn get(&mut self, _key: &CacheKey) -> Option<Vec<u8>> {
        None
    }

    fn put(&mut self, _key: CacheKey, _bytes: Vec<u8>) -> bool {
        false
    }

    fn clear(&mut self) {}

    fn stats(&self) -> CacheStats {
        self.stats
    }
}

#[derive(Debug)]
struct CacheEntry {
    bytes: Vec<u8>,
    weight: usize,
}

#[derive(Debug)]
pub struct MemoryCache {
    max_entries: usize,
    max_bytes: usize,
    entries: LruCache<CacheKey, CacheEntry>,
    bytes: usize,
    stats: CacheStats,
}

impl MemoryCache {
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        let capacity = NonZeroUsize::new(max_entries.max(1)).expect("capacity is non-zero");
        Self {
            max_entries,
            max_bytes,
            entries: LruCache::new(capacity),
            bytes: 0,
            stats: CacheStats::default(),
        }
    }

    fn evict_oldest(&mut self) {
        if let Some((_key, entry)) = self.entries.pop_lru() {
            self.bytes = self.bytes.saturating_sub(entry.weight);
            self.stats.evictions = self.stats.evictions.saturating_add(1);
        }
    }

    fn refresh_stats(&mut self) {
        self.stats.entries = self.entries.len();
        self.stats.bytes = self.bytes;
    }
}

impl ArtifactCache for MemoryCache {
    fn get(&mut self, key: &CacheKey) -> Option<Vec<u8>> {
        let value = self.entries.get(key).map(|entry| entry.bytes.clone());
        if value.is_some() {
            self.stats.hits = self.stats.hits.saturating_add(1);
        } else {
            self.stats.misses = self.stats.misses.saturating_add(1);
        }
        self.refresh_stats();
        value
    }

    fn put(&mut self, key: CacheKey, bytes: Vec<u8>) -> bool {
        let weight = key.weight().saturating_add(bytes.len());
        if self.max_entries == 0 || self.max_bytes == 0 || weight > self.max_bytes {
            return false;
        }

        if let Some(previous) = self.entries.pop(&key) {
            self.bytes = self.bytes.saturating_sub(previous.weight);
        }

        self.bytes = self.bytes.saturating_add(weight);
        if let Some((_evicted_key, evicted)) =
            self.entries.push(key.clone(), CacheEntry { bytes, weight })
        {
            self.bytes = self.bytes.saturating_sub(evicted.weight);
            self.stats.evictions = self.stats.evictions.saturating_add(1);
        }
        self.stats.insertions = self.stats.insertions.saturating_add(1);

        while self.entries.len() > self.max_entries || self.bytes > self.max_bytes {
            self.evict_oldest();
        }
        self.refresh_stats();
        self.entries.contains(&key)
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
        self.refresh_stats();
    }

    fn stats(&self) -> CacheStats {
        self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::{ArtifactCache, CacheKey, MemoryCache, NoopCache};
    use crate::model::{BlockKind, SemanticFormat};

    fn key(number: u8) -> CacheKey {
        CacheKey::new("test/renderer-v1", BlockKind::Math, &[number], 80, false, 0)
    }

    #[test]
    fn memory_cache_is_lru_and_bounded() {
        let mut cache = MemoryCache::new(2, 256);
        let first = key(1);
        let second = key(2);
        let third = key(3);
        assert!(cache.put(first.clone(), b"one".to_vec()));
        assert!(cache.put(second.clone(), b"two".to_vec()));
        assert_eq!(cache.get(&first), Some(b"one".to_vec()));
        assert!(cache.put(third.clone(), b"tri".to_vec()));
        assert_eq!(cache.get(&second), None);
        assert_eq!(cache.get(&first), Some(b"one".to_vec()));
        assert_eq!(cache.get(&third), Some(b"tri".to_vec()));
    }

    #[test]
    fn source_format_and_presentation_participate_in_cache_identity() {
        let tex = CacheKey::new_with_presentation(
            "test/renderer-v1",
            BlockKind::Math,
            SemanticFormat::Tex,
            b"same source",
            80,
            false,
            false,
            0,
        );
        let openmath = CacheKey::new_with_presentation(
            "test/renderer-v1",
            BlockKind::Math,
            SemanticFormat::OpenMath,
            b"same source",
            80,
            false,
            false,
            0,
        );
        let plain = CacheKey::new_with_presentation(
            "test/renderer-v1",
            BlockKind::Math,
            SemanticFormat::Tex,
            b"same source",
            80,
            false,
            true,
            0,
        );
        assert_ne!(tex, openmath);
        assert_ne!(tex, plain);
    }

    #[test]
    fn key_material_counts_toward_the_byte_limit() {
        let mut cache = MemoryCache::new(2, 8);
        assert!(!cache.put(key(1), b"x".to_vec()));
        assert_eq!(cache.stats().entries, 0);
    }

    #[test]
    fn replacement_updates_logical_bytes_without_an_eviction() {
        let mut cache = MemoryCache::new(2, 256);
        let first = key(1);
        assert!(cache.put(first.clone(), b"one".to_vec()));
        let before = cache.stats();
        assert!(cache.put(first.clone(), b"replacement".to_vec()));
        assert_eq!(cache.get(&first), Some(b"replacement".to_vec()));
        assert_eq!(cache.stats().evictions, before.evictions);
    }

    #[test]
    fn noop_cache_never_stores_or_reports_policy_misses() {
        let mut cache = NoopCache::default();
        let key = key(1);
        assert!(!cache.put(key.clone(), b"value".to_vec()));
        assert_eq!(cache.get(&key), None);
        assert_eq!(cache.stats().misses, 0);
    }
}
