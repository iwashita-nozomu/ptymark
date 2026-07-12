pub mod cache;
pub mod cli;
pub mod config;
pub mod detector;
pub mod engine;
pub mod model;
pub mod pipeline;
pub mod render;
pub mod terminal;

pub use cache::{ArtifactCache, CacheKey, CacheStats, MemoryCache, NoopCache};
pub use config::{
    CONFIG_SCHEMA_VERSION, CacheConfig, Config, ConfigError, DetectionConfig, EnginesConfig,
    MathEngine, MathEngineConfig, MermaidEngine, MermaidEngineConfig, PresenterConfig, RenderMode,
    RenderingConfig,
};
pub use detector::{FencedDetector, PassthroughDetector, SemanticDetector};
pub use engine::{
    ConfiguredRenderer, EngineCheck, check_configured_engines, resolve_executable,
};
pub use model::{BlockKind, SemanticBlock, StreamItem};
pub use pipeline::{DisplayPipeline, PipelineError, PipelineReport};
pub use render::{
    PreviewRenderer, RenderArtifact, RenderContext, RenderError, RenderOutput, RenderService,
    Renderer, SourceRenderer,
};
pub use terminal::{OutputSegment, TerminalOutputGate};