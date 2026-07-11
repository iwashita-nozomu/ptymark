pub mod artifact;
pub mod cache;
pub mod coordinator;
pub mod detector;
pub mod engine;
pub mod model;
pub mod predisplay;
pub mod presenter;
pub mod renderer;
pub mod terminal;
pub mod ui;

pub use artifact::{ArtifactFormat, EngineIdentity, RenderArtifact};
pub use cache::{
    ArtifactCache, ArtifactCacheKey, CacheAdmission, CachePolicy, CacheStats, InvalidationScope,
    MemoryArtifactCache, NoopArtifactCache, RenderCache, RenderKey,
};
pub use coordinator::{
    CacheDisposition, CoordinatorStats, EngineAttempt, RenderCoordinator, RenderOutcome,
};
pub use detector::{DetectError, FencedDetector, PassthroughDetector, SemanticDetector};
pub use engine::{
    EngineDescriptor, EngineRegistry, EngineSelector, ExecutionModel, PolicyEngineSelector,
    RenderEngine, RenderRequest,
};
pub use model::{BlockKind, DisplayMode, SemanticBlock, StreamItem};
pub use predisplay::{DisplayInterceptor, PreDisplayError, PreDisplayRenderer, PreDisplayReport};
pub use presenter::{
    ArtifactPresenter, SourcePresenter, TerminalCapabilities, TerminalTextPresenter,
};
pub use renderer::{
    BlockRenderer, CoordinatedRenderer, ExternalRenderer, ExternalRendererConfig, PreviewEngine,
    PreviewRenderer, RenderContext, RenderError, SourceEngine, SourceRenderer,
};
pub use terminal::{DisplayOutputGate, OutputSegment, TerminalOutputGate};
pub use ui::{LayoutSensitivity, ResizeAction, Viewport, resize_action, stable_fingerprint};
