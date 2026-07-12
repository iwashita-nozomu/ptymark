use crate::model::BlockKind;
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CacheKey {
    renderer_id: String,
    kind: BlockKind,
    source: Vec<u8>,
    columns: u16,
    color: bool,
    theme_fingerprint: u64,
}

impl CacheKey {
    pub fn new(
        renderer_id: impl Into<String>,
        kind: BlockKind,
        source: &[u8],
        columns: u16,
        color: bool,
        theme_fingerprint: u64,
    ) -> Self {
        Self {
            renderer_id: renderer_id.into(),
            kind,
            source: source.to_vec(),
            columns,
            color,
            theme_fingerprint,
        }
    }

    fn weight(&self) -> usize {
        self.renderer_id
            .len()
            .saturating_add(self.source.len())
            .saturating_add(std::mem::size_of::<BlockKind>())
            .saturating_add(std::mem::size_of::<u16>())
            .saturating_add(std::mem::size_of::<bool>())
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
        self.stats.misses = self.stats.misses.saturating_add(1);
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
    entries: HashMap<CacheKey, CacheEntry>,
    order: VecDeque<CacheKey>,
    bytes: usize,
    stats: CacheStats,
}

impl MemoryCache {
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            max_entries,
            max_bytes,
            entries: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            stats: CacheStats::default(),
        }
    }

    fn touch(&mut self, key: &CacheKey) {
        if let Some(index) = self.order.iter().position(|candidate| candidate == key) {
            self.order.remove(index);
        }
        self.order.push_back(key.clone());
    }

    fn evict_oldest(&mut self) {
        if let Some(key) = self.order.pop_front()
            && let Some(entry) = self.entries.remove(&key)
        {
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
            self.touch(key);
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

        if let Some(previous) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(previous.weight);
            if let Some(index) = self.order.iter().position(|candidate| candidate == &key) {
                self.order.remove(index);
            }
        }

        self.bytes = self.bytes.saturating_add(weight);
        self.entries
            .insert(key.clone(), CacheEntry { bytes, weight });
        self.order.push_back(key.clone());
        self.stats.insertions = self.stats.insertions.saturating_add(1);

        while self.entries.len() > self.max_entries || self.bytes > self.max_bytes {
            self.evict_oldest();
        }
        self.refresh_stats();
        self.entries.contains_key(&key)
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
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
    use crate::model::BlockKind;

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
    fn key_material_counts_toward_the_byte_limit() {
        let mut cache = MemoryCache::new(2, 8);
        assert!(!cache.put(key(1), b"x".to_vec()));
        assert_eq!(cache.stats().entries, 0);
    }

    #[test]
    fn noop_cache_never_stores() {
        let mut cache = NoopCache::default();
        let key = key(1);
        assert!(!cache.put(key.clone(), b"value".to_vec()));
        assert_eq!(cache.get(&key), None);
    }
}
