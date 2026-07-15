use crate::cache::{ArtifactCache, CacheKey, CacheStats};
use crate::diagnostics::{
    DiagnosticComponent, DiagnosticEvidence, DiagnosticFinding, DiagnosticSeverity, code,
};
use crate::model::SemanticBlock;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, Default)]
pub struct RenderCancellation {
    cancelled: Arc<AtomicBool>,
}

impl RenderCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderContext {
    pub columns: u16,
    pub color: bool,
    pub theme_fingerprint: u64,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            columns: 80,
            color: false,
            theme_fingerprint: 0,
        }
    }
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
    code: String,
    message: String,
}

impl RenderError {
    pub fn new(message: impl Into<String>) -> Self {
        Self::coded(code::RENDER_FAILED, message)
    }

    pub fn coded(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn diagnostic_finding(&self, strict: bool) -> DiagnosticFinding {
        let (component, summary, remedy) = match self.code.as_str() {
            code::ENGINE_MISSING => (
                DiagnosticComponent::Engine,
                "a configured renderer executable is unavailable",
                "install the configured executable, select preview/source, or use --safe",
            ),
            code::ENGINE_INCOMPATIBLE => (
                DiagnosticComponent::Engine,
                "a renderer or artifact is incompatible",
                "select a compatible renderer or use preview/source fallback",
            ),
            code::RENDER_TIMEOUT => (
                DiagnosticComponent::Render,
                "an external rendering attempt exceeded its hard deadline",
                "retry with source/safe mode or inspect the configured renderer",
            ),
            code::RENDER_OUTPUT_LIMIT => (
                DiagnosticComponent::Render,
                "an external renderer exceeded a bounded output limit",
                "reduce the block size or use source/safe mode",
            ),
            code::PRESENTATION_FALLBACK | code::PRESENTER_UNSUPPORTED => (
                DiagnosticComponent::Presentation,
                "the rendered artifact could not be presented safely",
                "install a supported presenter or use preview/source mode",
            ),
            _ => (
                DiagnosticComponent::Render,
                "an external rendering attempt failed",
                "inspect the renderer selection or use preview/source fallback",
            ),
        };
        DiagnosticFinding::new(
            self.code.clone(),
            if strict {
                DiagnosticSeverity::Error
            } else {
                DiagnosticSeverity::Warning
            },
            component,
            summary,
        )
        .with_remedy(remedy)
        .with_evidence("detail", DiagnosticEvidence::omitted())
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for RenderError {}

pub trait Renderer: Send {
    fn id(&self) -> &str;
    fn render(
        &mut self,
        block: &SemanticBlock,
        context: RenderContext,
    ) -> Result<RenderArtifact, RenderError>;
}

#[derive(Debug, Default)]
pub struct PreviewRenderer;

impl Renderer for PreviewRenderer {
    fn id(&self) -> &str {
        "builtin/preview-v1"
    }

    fn render(
        &mut self,
        block: &SemanticBlock,
        context: RenderContext,
    ) -> Result<RenderArtifact, RenderError> {
        let body = std::str::from_utf8(block.body()).map_err(|error| {
            RenderError::new(format!(
                "{} block is not valid UTF-8: {error}",
                block.kind()
            ))
        })?;

        let mut bytes = Vec::new();
        if context.color {
            bytes.extend_from_slice(b"\x1b[1;36m");
        }
        bytes.extend_from_slice(format!("┌─ ptymark {} ─\n", block.kind()).as_bytes());
        if context.color {
            bytes.extend_from_slice(b"\x1b[0m");
        }

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
    fn id(&self) -> &str {
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

    pub fn renderer_id(&self) -> &str {
        self.renderer.id()
    }
}

#[cfg(test)]
mod tests {
    use super::{PreviewRenderer, RenderContext, RenderService, Renderer, SourceRenderer};
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
    fn render_context_defaults_to_a_valid_terminal_width() {
        assert_eq!(RenderContext::default().columns, 80);
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
    fn invalid_utf8_is_rejected_before_preview_replacement() {
        let block = SemanticBlock::new(
            BlockKind::Math,
            b"$$\n\xff\n$$\n".to_vec(),
            b"\xff\n".to_vec(),
        );
        let error = PreviewRenderer
            .render(&block, RenderContext::default())
            .expect_err("invalid UTF-8 must not be rendered lossily");
        assert!(error.to_string().contains("not valid UTF-8"));
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
