pub mod artifact;
pub mod cache;
pub mod cli;
pub mod config;
pub mod coordinator;
pub mod detector;
pub mod engine;
pub mod fingerprint;
pub mod model;
pub mod predisplay;
pub mod presenter;
pub mod process_engine;
pub mod renderer;
pub mod runtime;
pub mod terminal;
pub mod ui;

pub use artifact::{ArtifactFormat, EngineIdentity, RenderArtifact};
pub use cache::{
    ArtifactCache, ArtifactCacheKey, CacheAdmission, CachePolicy, CacheStats, InvalidationScope,
    MemoryArtifactCache, NoopArtifactCache, RenderCache, RenderKey,
};
pub use config::{
    CONFIG_SCHEMA_VERSION, CacheBackend, CacheConfig, CachePolicyConfig, ConfigEnvironment,
    ConfigError, ConfigErrorKind, ConfigFile, ConfigLocator, ConfigManager, ConfigOrigin,
    ConfigProvenance, ConfigRequest, ConfigSnapshot, ConfigSource, ConfigTrust,
    ConfiguredExecutionModel, ConfiguredLayout, DetectionConfig, DetectionMode, DetectionPolicy,
    DiagnosticFormat, DiagnosticLevel, DiagnosticSink, DiagnosticsConfig, DiagnosticsPolicy,
    EngineSelectionConfig, EngineSelectionPolicy, EngineType, ExternalEngineConfig, FallbackPolicy,
    FenceConfig, FilesystemConfigLocator, LoadedConfig, PresentationConfig, PresentationMode,
    PresentationPolicy, ProfileConfig, RenderConfig, RenderOrdering, RenderPolicy,
    RendererBundleConfig, ResolvedConfig, RuntimeConfig, SessionMode, UnsupportedPresentation,
};
pub use coordinator::{
    CacheDisposition, CoordinatorStats, EngineAttempt, RenderCoordinator, RenderOutcome,
};
pub use detector::{
    DetectError, FencedDetector, FencedDetectorOptions, PassthroughDetector, SemanticDetector,
};
pub use engine::{
    EngineDescriptor, EngineRegistry, EngineSelector, ExecutionModel, PolicyEngineSelector,
    RenderEngine, RenderRequest,
};
pub use fingerprint::stable_fingerprint;
pub use model::{BlockKind, DisplayMode, SemanticBlock, StreamItem};
pub use predisplay::{DisplayInterceptor, PreDisplayError, PreDisplayRenderer, PreDisplayReport};
pub use presenter::{
    ArtifactPresenter, SourcePresenter, TerminalCapabilities, TerminalTextPresenter,
};
pub use process_engine::{ProcessEngine, ProcessEngineConfig, ProgramResolution};
pub use renderer::{
    BlockRenderer, CoordinatedRenderer, ExternalRenderer, ExternalRendererConfig, PreviewEngine,
    PreviewRenderer, RenderContext, RenderError, SourceEngine, SourceRenderer,
};
pub use runtime::{
    CacheProvider, DetectorProvider, EngineProvider, PresenterProvider, RuntimeBuildContext,
    RuntimeBuildError, RuntimeBuildReport, RuntimeBuilder, RuntimeMode, RuntimeRequest,
    SessionRuntime, UnavailableEngine,
};
pub use terminal::{DisplayOutputGate, OutputSegment, TerminalOutputGate};
pub use ui::{LayoutSensitivity, ResizeAction, Viewport, resize_action};
