pub mod cache;
pub mod cli;
mod command;
pub mod config;
mod config_ext;
pub mod detector;
pub mod diagnostics;
pub mod doctor;
pub mod engine;
mod filtered_run;
pub mod format_adapter;
pub mod install;
mod interactive;
mod limits;
pub mod managed_launcher;
pub mod model;
mod native_session;
pub mod openmath;
pub mod pipeline;
mod platform;
pub mod render;
pub mod routing;
pub mod runtime;
mod stream;
pub mod terminal;

pub use cache::{ArtifactCache, CacheKey, CacheStats, MemoryCache, NoopCache};
pub use config::{
    CONFIG_SCHEMA_VERSION, CacheBackend, CacheConfig, ColorPolicy, Config, ConfigError,
    DetectionConfig, EngineProvider, EngineSelection, EnginesConfig, MathEngine, MathEngineConfig,
    MermaidEngine, MermaidEngineConfig, PresentationMode, PresenterConfig, PresenterProvider,
    PresenterSelection, ProfileConfig, RenderMode, RenderingConfig, SessionConfig, SessionMode,
    UserCacheConfig, UserConfig, UserDetectionConfig, UserEnginesConfig, UserPresentationConfig,
};
pub use detector::{FencedDetector, PassthroughDetector, SemanticDetector};
pub use diagnostics::{
    DiagnosticComponent, DiagnosticEvidence, DiagnosticFinding, DiagnosticSeverity,
    DiagnosticStatus, Redactor,
};
pub use doctor::{DOCTOR_SCHEMA, DoctorReport, DoctorRequest};
pub use engine::{ConfiguredRenderer, EngineCheck, check_configured_engines, resolve_executable};
pub use format_adapter::OpenMathAdapterRenderer;
pub use install::{
    EnginePreference, INSTALL_STATE_SCHEMA_VERSION, InstallError, InstallPlan, InstallRequest,
    InstallState, InstalledComponent, Installer, PathProgramResolver, PresenterPreference,
    ProgramResolver, ResolutionOrigin, default_install_state_path,
};
pub use managed_launcher::{
    MANAGED_BUNDLE_SCHEMA_VERSION, ManagedBundleInspection, ManagedBundleStatus,
    inspect_managed_alias, run_if_managed_alias,
};
pub use model::{BlockKind, SemanticBlock, SemanticFormat, StreamItem};
pub use openmath::{
    OPENMATH_NAMESPACE, OPENMATH_TO_TEX_ID, OpenMathError, to_tex as openmath_to_tex,
};
pub use pipeline::{DisplayPipeline, MAX_PENDING_OUTPUT_BYTES, PipelineError, PipelineReport};
pub use render::{
    PreviewRenderer, RenderArtifact, RenderCancellation, RenderContext, RenderError, RenderOutput,
    RenderService, Renderer, SourceRenderer,
};
pub use routing::{
    ConfiguredDecider, ConfiguredHandoff, DecisionRequest, EngineHandoff, EngineRequest,
    EngineResponse, RenderDecider, RenderDecision, RenderRoute, RoutedRenderer,
};
pub use runtime::{PipelineFactory, PipelineOptions};
pub use terminal::{OutputSegment, TerminalOutputGate};
