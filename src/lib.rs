pub mod cache;
pub mod cli;
mod cli_args;
mod command;
pub mod config;
pub mod detector;
pub mod diagnostics;
pub mod doctor;
pub mod engine;
mod filtered_run;
pub mod install;
mod interactive;
pub mod managed_launcher;
pub mod model;
mod native_session;
pub mod pipeline;
pub mod render;
pub mod routing;
pub mod runtime;
mod stream;
pub mod terminal;

pub use cache::{ArtifactCache, CacheKey, CacheStats, MemoryCache, NoopCache};
pub use config::{
    CONFIG_SCHEMA_VERSION, CacheConfig, Config, ConfigError, DetectionConfig, EnginesConfig,
    MathEngine, MathEngineConfig, MermaidEngine, MermaidEngineConfig, PresenterConfig, RenderMode,
    RenderingConfig,
};
pub use detector::{FencedDetector, PassthroughDetector, SemanticDetector};
pub use diagnostics::{
    DiagnosticComponent, DiagnosticEvidence, DiagnosticFinding, DiagnosticSeverity,
    DiagnosticStatus, Redactor,
};
pub use doctor::{DOCTOR_SCHEMA, DoctorReport, DoctorRequest};
pub use engine::{ConfiguredRenderer, EngineCheck, check_configured_engines, resolve_executable};
pub use install::{
    EnginePreference, INSTALL_STATE_SCHEMA_VERSION, InstallError, InstallPlan, InstallRequest,
    InstallState, InstalledComponent, Installer, PathProgramResolver, PresenterPreference,
    ProgramResolver, ResolutionOrigin, default_install_state_path,
};
pub use managed_launcher::{
    MANAGED_BUNDLE_SCHEMA_VERSION, ManagedBundleInspection, inspect_managed_alias,
    run_if_managed_alias,
};
pub use model::{BlockKind, SemanticBlock, StreamItem};
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
