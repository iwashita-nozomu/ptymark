use crate::model::SemanticBlock;
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Viewport {
    pub columns: u16,
    pub rows: u16,
    pub pixel_width: Option<u32>,
    pub pixel_height: Option<u32>,
}

impl Viewport {
    pub const fn cells(columns: u16, rows: u16) -> Self {
        Self {
            columns,
            rows,
            pixel_width: None,
            pixel_height: None,
        }
    }

    pub const fn with_pixels(columns: u16, rows: u16, pixel_width: u32, pixel_height: u32) -> Self {
        Self {
            columns,
            rows,
            pixel_width: Some(pixel_width),
            pixel_height: Some(pixel_height),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LayoutSensitivity {
    #[default]
    Independent,
    Columns,
    Pixels,
    FullViewport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResizeAction {
    Reuse,
    Rerender,
}

pub fn resize_action(
    previous: Viewport,
    next: Viewport,
    sensitivity: LayoutSensitivity,
) -> ResizeAction {
    let changed = match sensitivity {
        LayoutSensitivity::Independent => false,
        LayoutSensitivity::Columns => previous.columns != next.columns,
        LayoutSensitivity::Pixels => match (
            previous.pixel_width,
            previous.pixel_height,
            next.pixel_width,
            next.pixel_height,
        ) {
            (Some(old_width), Some(old_height), Some(new_width), Some(new_height)) => {
                old_width != new_width || old_height != new_height
            }
            _ => previous.columns != next.columns || previous.rows != next.rows,
        },
        LayoutSensitivity::FullViewport => previous != next,
    };

    if changed {
        ResizeAction::Rerender
    } else {
        ResizeAction::Reuse
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RenderKey {
    source_fingerprint: u64,
    block_kind: &'static str,
    renderer_id: String,
    layout: LayoutKey,
    theme_fingerprint: u64,
    options_fingerprint: u64,
}

impl RenderKey {
    pub fn for_block(
        block: &SemanticBlock,
        renderer_id: impl Into<String>,
        viewport: Viewport,
        sensitivity: LayoutSensitivity,
        theme_fingerprint: u64,
        options_fingerprint: u64,
    ) -> Self {
        Self {
            source_fingerprint: stable_fingerprint(block.source()),
            block_kind: block.kind().as_str(),
            renderer_id: renderer_id.into(),
            layout: LayoutKey::from_viewport(viewport, sensitivity),
            theme_fingerprint,
            options_fingerprint,
        }
    }

    pub const fn source_fingerprint(&self) -> u64 {
        self.source_fingerprint
    }

    pub const fn theme_fingerprint(&self) -> u64 {
        self.theme_fingerprint
    }

    pub fn renderer_id(&self) -> &str {
        &self.renderer_id
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
    Full(ViewportKey),
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
            LayoutSensitivity::FullViewport => Self::Full(ViewportKey::from(viewport)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ViewportKey {
    columns: u16,
    rows: u16,
    pixel_width: Option<u32>,
    pixel_height: Option<u32>,
}

impl From<Viewport> for ViewportKey {
    fn from(viewport: Viewport) -> Self {
        Self {
            columns: viewport.columns,
            rows: viewport.rows,
            pixel_width: viewport.pixel_width,
            pixel_height: viewport.pixel_height,
        }
    }
}

pub fn stable_fingerprint(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001b3;

    bytes.iter().fold(OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
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

#[derive(Debug)]
pub struct RenderCache {
    policy: CachePolicy,
    entries: HashMap<RenderKey, Vec<u8>>,
    order: VecDeque<RenderKey>,
    bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    rejected: u64,
}

impl RenderCache {
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

    pub fn get(&mut self, key: &RenderKey) -> Option<Vec<u8>> {
        let value = self.entries.get(key).cloned();
        if value.is_some() {
            self.hits = self.hits.saturating_add(1);
            self.touch(key);
        } else {
            self.misses = self.misses.saturating_add(1);
        }
        value
    }

    pub fn insert(&mut self, key: RenderKey, bytes: Vec<u8>) -> bool {
        if self.policy.max_entries == 0
            || self.policy.max_bytes == 0
            || bytes.len() > self.policy.max_bytes
        {
            self.rejected = self.rejected.saturating_add(1);
            return false;
        }

        if let Some(previous) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(previous.len());
            self.remove_from_order(&key);
        }

        self.bytes = self.bytes.saturating_add(bytes.len());
        self.entries.insert(key.clone(), bytes);
        self.order.push_back(key);
        self.evict_to_policy();
        true
    }

    pub fn invalidate_theme(&mut self, theme_fingerprint: u64) {
        let keys: Vec<RenderKey> = self
            .entries
            .keys()
            .filter(|key| key.theme_fingerprint() == theme_fingerprint)
            .cloned()
            .collect();
        for key in keys {
            self.remove(&key);
        }
    }

    pub fn invalidate_renderer(&mut self, renderer_id: &str) {
        let keys: Vec<RenderKey> = self
            .entries
            .keys()
            .filter(|key| key.renderer_id() == renderer_id)
            .cloned()
            .collect();
        for key in keys {
            self.remove(&key);
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.bytes = 0;
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.entries.len(),
            bytes: self.bytes,
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            rejected: self.rejected,
        }
    }

    fn touch(&mut self, key: &RenderKey) {
        self.remove_from_order(key);
        self.order.push_back(key.clone());
    }

    fn remove(&mut self, key: &RenderKey) {
        if let Some(bytes) = self.entries.remove(key) {
            self.bytes = self.bytes.saturating_sub(bytes.len());
            self.remove_from_order(key);
        }
    }

    fn remove_from_order(&mut self, key: &RenderKey) {
        if let Some(index) = self.order.iter().position(|candidate| candidate == key) {
            self.order.remove(index);
        }
    }

    fn evict_to_policy(&mut self) {
        while self.entries.len() > self.policy.max_entries || self.bytes > self.policy.max_bytes {
            let Some(key) = self.order.pop_front() else {
                break;
            };
            if let Some(bytes) = self.entries.remove(&key) {
                self.bytes = self.bytes.saturating_sub(bytes.len());
                self.evictions = self.evictions.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CachePolicy, LayoutSensitivity, RenderCache, RenderKey, ResizeAction, Viewport,
        resize_action,
    };
    use crate::model::{BlockKind, SemanticBlock};

    fn block(source: &[u8]) -> SemanticBlock {
        SemanticBlock::new(BlockKind::Math, source.to_vec(), source.to_vec())
    }

    fn key(source: &[u8], width: u16, theme: u64) -> RenderKey {
        RenderKey::for_block(
            &block(source),
            "test/renderer",
            Viewport::cells(width, 24),
            LayoutSensitivity::Columns,
            theme,
            0,
        )
    }

    #[test]
    fn column_layout_ignores_height_only_changes() {
        assert_eq!(
            resize_action(
                Viewport::cells(80, 24),
                Viewport::cells(80, 50),
                LayoutSensitivity::Columns,
            ),
            ResizeAction::Reuse
        );
        assert_eq!(
            resize_action(
                Viewport::cells(80, 24),
                Viewport::cells(120, 24),
                LayoutSensitivity::Columns,
            ),
            ResizeAction::Rerender
        );
    }

    #[test]
    fn pixel_layout_tracks_pixel_geometry() {
        assert_eq!(
            resize_action(
                Viewport::with_pixels(80, 24, 800, 480),
                Viewport::with_pixels(80, 24, 1000, 480),
                LayoutSensitivity::Pixels,
            ),
            ResizeAction::Rerender
        );
    }

    #[test]
    fn render_key_changes_with_width_and_theme() {
        assert_ne!(key(b"same", 80, 1), key(b"same", 120, 1));
        assert_ne!(key(b"same", 80, 1), key(b"same", 80, 2));
        assert_eq!(key(b"same", 80, 1), key(b"same", 80, 1));
    }

    #[test]
    fn cache_is_lru_and_bounded_by_entry_count() {
        let mut cache = RenderCache::new(CachePolicy::new(2, 1024));
        let first = key(b"one", 80, 1);
        let second = key(b"two", 80, 1);
        let third = key(b"three", 80, 1);

        assert!(cache.insert(first.clone(), b"1".to_vec()));
        assert!(cache.insert(second.clone(), b"2".to_vec()));
        assert_eq!(cache.get(&first), Some(b"1".to_vec()));
        assert!(cache.insert(third.clone(), b"3".to_vec()));

        assert_eq!(cache.get(&second), None);
        assert_eq!(cache.get(&first), Some(b"1".to_vec()));
        assert_eq!(cache.get(&third), Some(b"3".to_vec()));
        assert_eq!(cache.stats().entries, 2);
        assert_eq!(cache.stats().evictions, 1);
    }

    #[test]
    fn cache_rejects_single_entry_larger_than_budget() {
        let mut cache = RenderCache::new(CachePolicy::new(4, 3));
        assert!(!cache.insert(key(b"one", 80, 1), b"1234".to_vec()));
        assert_eq!(cache.stats().entries, 0);
        assert_eq!(cache.stats().rejected, 1);
    }

    #[test]
    fn theme_invalidation_removes_only_matching_generation() {
        let mut cache = RenderCache::new(CachePolicy::new(4, 1024));
        let old = key(b"same", 80, 1);
        let current = key(b"same", 80, 2);
        cache.insert(old.clone(), b"old".to_vec());
        cache.insert(current.clone(), b"current".to_vec());

        cache.invalidate_theme(1);
        assert_eq!(cache.get(&old), None);
        assert_eq!(cache.get(&current), Some(b"current".to_vec()));
    }
}
