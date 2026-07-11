use crate::artifact::{ArtifactFormat, RenderArtifact};
use crate::model::{BlockKind, SemanticBlock};
use crate::ui::{LayoutSensitivity, Viewport, stable_fingerprint};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ArtifactCacheKey {
    source_fingerprint: u64,
    block_kind: BlockKind,
    engine_id: String,
    engine_version: String,
    artifact_format: ArtifactFormat,
    layout: LayoutKey,
    theme_fingerprint: u64,
    options_fingerprint: u64,
    presenter_id: String,
    capability_fingerprint: u64,
}

impl ArtifactCacheKey {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        block: &SemanticBlock,
        engine_id: impl Into<String>,
        engine_version: impl Into<String>,
        artifact_format: ArtifactFormat,
        viewport: Viewport,
        sensitivity: LayoutSensitivity,
        theme_fingerprint: u64,
        options_fingerprint: u64,
        presenter_id: impl Into<String>,
        capability_fingerprint: u64,
    ) -> Self {
        Self {
            source_fingerprint: stable_fingerprint(block.source()),
            block_kind: block.kind(),
            engine_id: engine_id.into(),
            engine_version: engine_version.into(),
            artifact_format,
            layout: LayoutKey::from_viewport(viewport, sensitivity),
            theme_fingerprint,
            options_fingerprint,
            presenter_id: presenter_id.into(),
            capability_fingerprint,
        }
    }

    pub fn for_block(
        block: &SemanticBlock,
        renderer_id: impl Into<String>,
        viewport: Viewport,
        sensitivity: LayoutSensitivity,
        theme_fingerprint: u64,
        options_fingerprint: u64,
    ) -> Self {
        Self::new(
            block,
            renderer_id,
            "legacy",
            ArtifactFormat::TerminalText,
            viewport,
            sensitivity,
            theme_fingerprint,
            options_fingerprint,
            "legacy",
            0,
        )
    }

    pub const fn source_fingerprint(&self) -> u64 {
        self.source_fingerprint
    }

    pub const fn theme_fingerprint(&self) -> u64 {
        self.theme_fingerprint
    }

    pub fn renderer_id(&self) -> &str {
        &self.engine_id
    }

    pub fn engine_id(&self) -> &str {
        &self.engine_id
    }

    pub fn engine_version(&self) -> &str {
        &self.engine_version
    }

    pub fn presenter_id(&self) -> &str {
        &self.presenter_id
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum LayoutKey {
    Independent,
    Columns(u16),
    Pixels {
        width: u32,
        height: u32,
        fallback_columns: u16,
        fallback_rows: u16,
    },
    Full(Viewport),
}

impl LayoutKey {
    fn from_viewport(viewport: Viewport, sensitivity: LayoutSensitivity) -> Self {
        match sensitivity {
            LayoutSensitivity::Independent => Self::Independent,
            LayoutSensitivity::Columns => Self::Columns(viewport.columns),
            LayoutSensitivity::Pixels => Self::Pixels {
                width: viewport.pixel_width.unwrap_or_default(),
                height: viewport.pixel_height.unwrap_or_default(),
                fallback_columns: viewport.columns,
                fallback_rows: viewport.rows,
            },
            LayoutSensitivity::FullViewport => Self::Full(viewport),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CachePolicy {
    pub max_entries: usize,
    pub max_bytes: usize,
}

impl CachePolicy {
    pub const fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            max_entries,
            max_bytes,
        }
    }
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self::new(128, 32 * 1024 * 1024)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CacheStats {
    pub entries: usize,
    pub bytes: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub rejected: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheAdmission {
    Stored,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidationScope {
    All,
    Engine(String),
    EngineVersion { id: String, version: String },
    Theme(u64),
    Presenter(String),
}

pub trait ArtifactCache: Send {
    fn get(&mut self, key: &ArtifactCacheKey) -> Option<RenderArtifact>;
    fn insert(&mut self, key: ArtifactCacheKey, artifact: RenderArtifact) -> CacheAdmission;
    fn invalidate(&mut self, scope: &InvalidationScope) -> usize;
    fn clear(&mut self);
    fn stats(&self) -> CacheStats;
}

#[derive(Debug, Default)]
pub struct NoopArtifactCache {
    hits: u64,
    misses: u64,
    rejected: u64,
}

impl ArtifactCache for NoopArtifactCache {
    fn get(&mut self, _key: &ArtifactCacheKey) -> Option<RenderArtifact> {
        self.misses = self.misses.saturating_add(1);
        None
    }

    fn insert(&mut self, _key: ArtifactCacheKey, _artifact: RenderArtifact) -> CacheAdmission {
        self.rejected = self.rejected.saturating_add(1);
        CacheAdmission::Rejected
    }

    fn invalidate(&mut self, _scope: &InvalidationScope) -> usize {
        0
    }

    fn clear(&mut self) {}

    fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            rejected: self.rejected,
            ..CacheStats::default()
        }
    }
}

#[derive(Debug)]
pub struct MemoryArtifactCache {
    policy: CachePolicy,
    entries: HashMap<ArtifactCacheKey, RenderArtifact>,
    order: VecDeque<ArtifactCacheKey>,
    bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    rejected: u64,
}

impl MemoryArtifactCache {
    pub fn new(policy: CachePolicy) -> Self {
        Self {
            policy,
            entries: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            rejected: 0,
        }
    }

    fn touch(&mut self, key: &ArtifactCacheKey) {
        self.remove_from_order(key);
        self.order.push_back(key.clone());
    }

    fn remove(&mut self, key: &ArtifactCacheKey) -> Option<RenderArtifact> {
        let artifact = self.entries.remove(key)?;
        self.bytes = self.bytes.saturating_sub(artifact.bytes.len());
        self.remove_from_order(key);
        Some(artifact)
    }

    fn remove_from_order(&mut self, key: &ArtifactCacheKey) {
        if let Some(index) = self.order.iter().position(|candidate| candidate == key) {
            self.order.remove(index);
        }
    }

    fn evict_to_policy(&mut self) {
        while self.entries.len() > self.policy.max_entries || self.bytes > self.policy.max_bytes {
            let Some(key) = self.order.pop_front() else {
                break;
            };
            if let Some(artifact) = self.entries.remove(&key) {
                self.bytes = self.bytes.saturating_sub(artifact.bytes.len());
                self.evictions = self.evictions.saturating_add(1);
            }
        }
    }
}

impl ArtifactCache for MemoryArtifactCache {
    fn get(&mut self, key: &ArtifactCacheKey) -> Option<RenderArtifact> {
        let value = self.entries.get(key).cloned();
        if value.is_some() {
            self.hits = self.hits.saturating_add(1);
            self.touch(key);
        } else {
            self.misses = self.misses.saturating_add(1);
        }
        value
    }

    fn insert(&mut self, key: ArtifactCacheKey, artifact: RenderArtifact) -> CacheAdmission {
        if !artifact.cacheable
            || self.policy.max_entries == 0
            || self.policy.max_bytes == 0
            || artifact.bytes.len() > self.policy.max_bytes
        {
            self.rejected = self.rejected.saturating_add(1);
            return CacheAdmission::Rejected;
        }

        let _ = self.remove(&key);
        self.bytes = self.bytes.saturating_add(artifact.bytes.len());
        self.entries.insert(key.clone(), artifact);
        self.order.push_back(key);
        self.evict_to_policy();
        CacheAdmission::Stored
    }

    fn invalidate(&mut self, scope: &InvalidationScope) -> usize {
        if matches!(scope, InvalidationScope::All) {
            let removed = self.entries.len();
            self.clear();
            return removed;
        }

        let keys: Vec<ArtifactCacheKey> = self
            .entries
            .keys()
            .filter(|key| match scope {
                InvalidationScope::All => true,
                InvalidationScope::Engine(id) => key.engine_id() == id,
                InvalidationScope::EngineVersion { id, version } => {
                    key.engine_id() == id && key.engine_version() == version
                }
                InvalidationScope::Theme(theme) => key.theme_fingerprint() == *theme,
                InvalidationScope::Presenter(id) => key.presenter_id() == id,
            })
            .cloned()
            .collect();

        let removed = keys.len();
        for key in keys {
            let _ = self.remove(&key);
        }
        removed
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.bytes = 0;
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.entries.len(),
            bytes: self.bytes,
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            rejected: self.rejected,
        }
    }
}

pub type RenderKey = ArtifactCacheKey;
pub type RenderCache = MemoryArtifactCache;

#[cfg(test)]
mod tests {
    use super::{
        ArtifactCache, ArtifactCacheKey, CacheAdmission, CachePolicy, InvalidationScope,
        MemoryArtifactCache, NoopArtifactCache,
    };
    use crate::artifact::{ArtifactFormat, EngineIdentity, RenderArtifact};
    use crate::model::{BlockKind, SemanticBlock};
    use crate::ui::{LayoutSensitivity, Viewport};

    fn block(source: &[u8]) -> SemanticBlock {
        SemanticBlock::new(BlockKind::Math, source.to_vec(), source.to_vec())
    }

    fn key(source: &[u8], width: u16, theme: u64) -> ArtifactCacheKey {
        ArtifactCacheKey::new(
            &block(source),
            "test/renderer",
            "1",
            ArtifactFormat::Svg,
            Viewport::cells(width, 24),
            LayoutSensitivity::Columns,
            theme,
            0,
            "terminal",
            1,
        )
    }

    fn artifact(bytes: &[u8]) -> RenderArtifact {
        RenderArtifact::new(
            ArtifactFormat::Svg,
            bytes.to_vec(),
            EngineIdentity::new("test/renderer", "1"),
            BlockKind::Math,
            LayoutSensitivity::Columns,
        )
    }

    #[test]
    fn cache_is_lru_and_bounded_by_entry_count() {
        let mut cache = MemoryArtifactCache::new(CachePolicy::new(2, 1024));
        let first = key(b"one", 80, 1);
        let second = key(b"two", 80, 1);
        let third = key(b"three", 80, 1);

        assert_eq!(
            cache.insert(first.clone(), artifact(b"1")),
            CacheAdmission::Stored
        );
        assert_eq!(
            cache.insert(second.clone(), artifact(b"2")),
            CacheAdmission::Stored
        );
        assert_eq!(
            cache.get(&first).map(|value| value.bytes),
            Some(b"1".to_vec())
        );
        assert_eq!(
            cache.insert(third.clone(), artifact(b"3")),
            CacheAdmission::Stored
        );

        assert!(cache.get(&second).is_none());
        assert_eq!(
            cache.get(&first).map(|value| value.bytes),
            Some(b"1".to_vec())
        );
        assert_eq!(
            cache.get(&third).map(|value| value.bytes),
            Some(b"3".to_vec())
        );
        assert_eq!(cache.stats().entries, 2);
        assert_eq!(cache.stats().evictions, 1);
    }

    #[test]
    fn cache_key_tracks_width_theme_engine_and_presenter() {
        assert_ne!(key(b"same", 80, 1), key(b"same", 120, 1));
        assert_ne!(key(b"same", 80, 1), key(b"same", 80, 2));
        assert_eq!(key(b"same", 80, 1), key(b"same", 80, 1));
    }

    #[test]
    fn invalidation_is_scoped() {
        let mut cache = MemoryArtifactCache::new(CachePolicy::default());
        let first = key(b"one", 80, 1);
        let second = key(b"two", 80, 2);
        cache.insert(first.clone(), artifact(b"1"));
        cache.insert(second.clone(), artifact(b"2"));

        assert_eq!(cache.invalidate(&InvalidationScope::Theme(1)), 1);
        assert!(cache.get(&first).is_none());
        assert!(cache.get(&second).is_some());
    }

    #[test]
    fn noop_cache_is_a_drop_in_private_mode() {
        let mut cache = NoopArtifactCache::default();
        let key = key(b"private", 80, 1);
        assert!(cache.get(&key).is_none());
        assert_eq!(
            cache.insert(key, artifact(b"secret")),
            CacheAdmission::Rejected
        );
        assert_eq!(cache.stats().entries, 0);
        assert_eq!(cache.stats().misses, 1);
    }
}
