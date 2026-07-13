use crate::cache::{ArtifactCache, MemoryCache, NoopCache};
use crate::config::{Config, RenderMode};
use crate::detector::{FencedDetector, PassthroughDetector, SemanticDetector};
use crate::pipeline::DisplayPipeline;
use crate::render::{RenderContext, RenderService, Renderer, SourceRenderer};
use crate::routing::RoutedRenderer;

/// Per-invocation overrides applied on top of the resolved user configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PipelineOptions {
    pub source: bool,
    pub strict: bool,
    pub no_cache: bool,
    pub color: bool,
    pub columns: Option<u16>,
    pub theme_fingerprint: u64,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            source: false,
            strict: false,
            no_cache: false,
            color: false,
            columns: None,
            theme_fingerprint: 0,
        }
    }
}

/// Canonical composition root for the pre-display pipeline.
///
/// Preview, pipe-based execution, and PTY/ConPTY execution must use this
/// factory rather than selecting detectors, engines, or cache implementations
/// independently.
#[derive(Clone, Copy, Debug)]
pub struct PipelineFactory<'a> {
    config: &'a Config,
}

impl<'a> PipelineFactory<'a> {
    pub const fn new(config: &'a Config) -> Self {
        Self { config }
    }

    pub fn build(self, options: PipelineOptions) -> DisplayPipeline {
        let detector: Box<dyn SemanticDetector> =
            if self.config.detection.math || self.config.detection.mermaid {
                Box::new(FencedDetector::new(&self.config.detection))
            } else {
                Box::new(PassthroughDetector)
            };

        let source_mode = options.source || self.config.rendering.mode == RenderMode::Source;
        let renderer: Box<dyn Renderer> = if source_mode {
            Box::new(SourceRenderer)
        } else {
            Box::new(RoutedRenderer::configured(&self.config.engines))
        };
        let cache: Box<dyn ArtifactCache> =
            if source_mode || options.no_cache || !self.config.cache.enabled {
                Box::new(NoopCache::default())
            } else {
                Box::new(MemoryCache::new(
                    self.config.cache.max_entries,
                    self.config.cache.max_bytes,
                ))
            };

        DisplayPipeline::new(
            detector,
            RenderService::new(renderer, cache),
            RenderContext {
                columns: options
                    .columns
                    .unwrap_or(self.config.rendering.columns)
                    .max(1),
                color: options.color,
                theme_fingerprint: options.theme_fingerprint,
            },
            options.strict || self.config.rendering.strict,
        )
    }
}
