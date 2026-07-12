use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CacheKey(pub u64);

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
    fn get(&mut self, key: CacheKey) -> Option<Vec<u8>>;
    fn put(&mut self, key: CacheKey, bytes: Vec<u8>) -> bool;
    fn clear(&mut self);
    fn stats(&self) -> CacheStats;
}

#[derive(Debug, Default)]
pub struct NoopCache {
    stats: CacheStats,
}

impl ArtifactCache for NoopCache {
    fn get(&mut self, _key: CacheKey) -> Option<Vec<u8>> {
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
pub struct MemoryCache {
    max_entries: usize,
    max_bytes: usize,
    entries: HashMap<CacheKey, Vec<u8>>,
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

    fn touch(&mut self, key: CacheKey) {
        if let Some(index) = self.order.iter().position(|candidate| *candidate == key) {
            self.order.remove(index);
        }
        self.order.push_back(key);
    }

    fn evict_oldest(&mut self) {
        if let Some(key) = self.order.pop_front()
            && let Some(bytes) = self.entries.remove(&key)
        {
            self.bytes = self.bytes.saturating_sub(bytes.len());
            self.stats.evictions = self.stats.evictions.saturating_add(1);
        }
    }

    fn refresh_stats(&mut self) {
        self.stats.entries = self.entries.len();
        self.stats.bytes = self.bytes;
    }
}

impl ArtifactCache for MemoryCache {
    fn get(&mut self, key: CacheKey) -> Option<Vec<u8>> {
        let value = self.entries.get(&key).cloned();
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
        if self.max_entries == 0 || self.max_bytes == 0 || bytes.len() > self.max_bytes {
            return false;
        }

        if let Some(previous) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(previous.len());
            if let Some(index) = self.order.iter().position(|candidate| *candidate == key) {
                self.order.remove(index);
            }
        }

        self.bytes = self.bytes.saturating_add(bytes.len());
        self.entries.insert(key, bytes);
        self.order.push_back(key);
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

    #[test]
    fn memory_cache_is_lru_and_bounded() {
        let mut cache = MemoryCache::new(2, 8);
        assert!(cache.put(CacheKey(1), b"one".to_vec()));
        assert!(cache.put(CacheKey(2), b"two".to_vec()));
        assert_eq!(cache.get(CacheKey(1)), Some(b"one".to_vec()));
        assert!(cache.put(CacheKey(3), b"tri".to_vec()));
        assert_eq!(cache.get(CacheKey(2)), None);
        assert_eq!(cache.get(CacheKey(1)), Some(b"one".to_vec()));
        assert_eq!(cache.get(CacheKey(3)), Some(b"tri".to_vec()));
    }

    #[test]
    fn oversized_entries_are_rejected() {
        let mut cache = MemoryCache::new(2, 3);
        assert!(!cache.put(CacheKey(1), b"four".to_vec()));
        assert_eq!(cache.stats().entries, 0);
    }

    #[test]
    fn noop_cache_never_stores() {
        let mut cache = NoopCache::default();
        assert!(!cache.put(CacheKey(1), b"value".to_vec()));
        assert_eq!(cache.get(CacheKey(1)), None);
    }
}
