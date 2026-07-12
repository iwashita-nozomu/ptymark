use crate::cache::{ArtifactCache, CacheKey, CacheStats};
use crate::model::SemanticBlock;
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderContext {
    pub columns: u16,
    pub color: bool,
    pub theme_fingerprint: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderArtifact {
    pub bytes: Vec<u8>,
    pub cacheable: bool,
}

impl RenderArtifact {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            cacheable: true,
        }
    }

    pub fn not_cacheable(mut self) -> Self {
        self.cacheable = false;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderError {
    message: String,
}

impl RenderError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for RenderError {}

pub trait Renderer: Send {
    fn id(&self) -> &'static str;
    fn render(
        &mut self,
        block: &SemanticBlock,
        context: RenderContext,
    ) -> Result<RenderArtifact, RenderError>;
}

#[derive(Debug, Default)]
pub struct PreviewRenderer;

impl Renderer for PreviewRenderer {
    fn id(&self) -> &'static str {
        "builtin/preview-v1"
    }

    fn render(
        &mut self,
        block: &SemanticBlock,
        context: RenderContext,
    ) -> Result<RenderArtifact, RenderError> {
        let mut bytes = Vec::new();
        if context.color {
            bytes.extend_from_slice(b"\x1b[1;36m");
        }
        bytes.extend_from_slice(format!("┌─ ptymark {} ─\n", block.kind()).as_bytes());
        if context.color {
            bytes.extend_from_slice(b"\x1b[0m");
        }

        let body = String::from_utf8_lossy(block.body());
        if body.is_empty() {
            bytes.extend_from_slice("│ <empty>\n".as_bytes());
        } else {
            for line in body.lines() {
                bytes.extend_from_slice("│ ".as_bytes());
                bytes.extend_from_slice(line.as_bytes());
                bytes.push(b'\n');
            }
        }
        bytes.extend_from_slice("└─\n".as_bytes());
        Ok(RenderArtifact::new(bytes))
    }
}

#[derive(Debug, Default)]
pub struct SourceRenderer;

impl Renderer for SourceRenderer {
    fn id(&self) -> &'static str {
        "builtin/source-v1"
    }

    fn render(
        &mut self,
        block: &SemanticBlock,
        _context: RenderContext,
    ) -> Result<RenderArtifact, RenderError> {
        Ok(RenderArtifact::new(block.source().to_vec()).not_cacheable())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderOutput {
    pub bytes: Vec<u8>,
    pub cache_hit: bool,
}

pub struct RenderService {
    renderer: Box<dyn Renderer>,
    cache: Box<dyn ArtifactCache>,
}

impl RenderService {
    pub fn new(renderer: Box<dyn Renderer>, cache: Box<dyn ArtifactCache>) -> Self {
        Self { renderer, cache }
    }

    pub fn render(
        &mut self,
        block: &SemanticBlock,
        context: RenderContext,
    ) -> Result<RenderOutput, RenderError> {
        let key = CacheKey::new(
            self.renderer.id(),
            block.kind(),
            block.source(),
            context.columns,
            context.color,
            context.theme_fingerprint,
        );
        if let Some(bytes) = self.cache.get(&key) {
            return Ok(RenderOutput {
                bytes,
                cache_hit: true,
            });
        }

        let artifact = self.renderer.render(block, context)?;
        if artifact.cacheable {
            self.cache.put(key, artifact.bytes.clone());
        }
        Ok(RenderOutput {
            bytes: artifact.bytes,
            cache_hit: false,
        })
    }

    pub fn cache_stats(&self) -> CacheStats {
        self.cache.stats()
    }

    pub fn renderer_id(&self) -> &'static str {
        self.renderer.id()
    }
}

#[cfg(test)]
mod tests {
    use super::{PreviewRenderer, RenderContext, RenderService, SourceRenderer};
    use crate::cache::{MemoryCache, NoopCache};
    use crate::model::{BlockKind, SemanticBlock};

    fn block() -> SemanticBlock {
        SemanticBlock::new(
            BlockKind::Math,
            b"$$\nE = mc^2\n$$\n".to_vec(),
            b"E = mc^2\n".to_vec(),
        )
    }

    #[test]
    fn service_caches_successful_artifacts() {
        let mut service = RenderService::new(
            Box::new(PreviewRenderer),
            Box::new(MemoryCache::new(8, 4096)),
        );
        let first = service
            .render(&block(), RenderContext::default())
            .expect("first");
        let second = service
            .render(&block(), RenderContext::default())
            .expect("second");
        assert!(!first.cache_hit);
        assert!(second.cache_hit);
    }

    #[test]
    fn source_renderer_is_exact() {
        let block = block();
        let mut service =
            RenderService::new(Box::new(SourceRenderer), Box::new(NoopCache::default()));
        let output = service
            .render(&block, RenderContext::default())
            .expect("source");
        assert_eq!(output.bytes, block.source());
    }
}
