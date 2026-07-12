use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionMode {
    #[default]
    Transform,
    Source,
    Bypass,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FallbackPolicy {
    #[default]
    Source,
    Error,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DetectionMode {
    Off,
    #[default]
    ExplicitBlocks,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PresentationMode {
    #[default]
    Auto,
    Text,
    Source,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnsupportedPresentation {
    #[default]
    Source,
    Text,
    ErrorBeforeLaunch,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderOrdering {
    #[default]
    Strict,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheBackend {
    None,
    #[default]
    Memory,
    Disk,
    Tiered,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticLevel {
    Off,
    Error,
    #[default]
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticFormat {
    #[default]
    Text,
    JsonLines,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSink {
    #[default]
    Stderr,
    File,
    Both,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineType {
    #[default]
    Process,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfiguredExecutionModel {
    #[default]
    OneShot,
    PersistentWorker,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfiguredLayout {
    Independent,
    Columns,
    #[default]
    Pixels,
    FullViewport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    pub schema_version: u32,
    #[serde(default)]
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfileConfig>,
    #[serde(default)]
    pub engines: BTreeMap<String, ExternalEngineConfig>,
    #[serde(default)]
    pub runtimes: BTreeMap<String, RuntimeConfig>,
    #[serde(default)]
    pub renderer_bundle: RendererBundleConfig,
    #[serde(default)]
    pub diagnostics: DiagnosticsConfig,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProfileConfig {
    pub extends: Option<String>,
    pub mode: Option<SessionMode>,
    pub strict: Option<bool>,
    pub fallback: Option<FallbackPolicy>,
    pub detection: DetectionConfig,
    pub render: RenderConfig,
    pub presentation: PresentationConfig,
    pub cache: CacheConfig,
    pub diagnostics: DiagnosticsConfig,
    pub engines: BTreeMap<String, EngineSelectionConfig>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DetectionConfig {
    pub mode: Option<DetectionMode>,
    pub mermaid: Option<bool>,
    pub block_math: Option<bool>,
    pub max_buffer_bytes: Option<usize>,
    pub max_line_bytes: Option<usize>,
    pub fences: FenceConfig,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FenceConfig {
    pub mermaid: Option<Vec<String>>,
    pub math: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EngineSelectionConfig {
    pub candidates: Option<Vec<String>>,
    pub preferred_artifacts: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RenderConfig {
    pub soft_latency_budget_ms: Option<u64>,
    pub hard_timeout_ms: Option<u64>,
    pub max_in_flight: Option<usize>,
    pub ordering: Option<RenderOrdering>,
    pub prewarm: Option<bool>,
    pub worker_idle_ms: Option<u64>,
    pub worker_max_requests: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PresentationConfig {
    pub mode: Option<PresentationMode>,
    pub prefer: Option<Vec<String>>,
    pub image_protocols: Option<Vec<String>>,
    pub unsupported: Option<UnsupportedPresentation>,
    pub transparent_background: Option<bool>,
    pub max_columns: Option<u16>,
    pub max_rows: Option<u16>,
    pub preserve_aspect_ratio: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CacheConfig {
    pub backend: Option<CacheBackend>,
    pub max_entries: Option<usize>,
    pub max_bytes: Option<usize>,
    pub ttl_seconds: Option<u64>,
    pub path: Option<PathBuf>,
    pub private: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DiagnosticsConfig {
    pub level: Option<DiagnosticLevel>,
    pub format: Option<DiagnosticFormat>,
    pub sink: Option<DiagnosticSink>,
    pub path: Option<PathBuf>,
    pub include_source: Option<bool>,
    pub metrics: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalEngineConfig {
    #[serde(rename = "type")]
    pub engine_type: EngineType,
    pub version: String,
    pub semantic_kinds: Vec<String>,
    pub artifact_types: Vec<String>,
    pub layout: ConfiguredLayout,
    pub execution: ConfiguredExecutionModel,
    pub program: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_engine_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_engine_stdout_bytes")]
    pub max_stdout_bytes: usize,
    #[serde(default = "default_engine_stderr_bytes")]
    pub max_stderr_bytes: usize,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub inherit_environment: Vec<String>,
    #[serde(default)]
    pub working_directory: Option<PathBuf>,
}

const fn default_engine_timeout_ms() -> u64 {
    1_500
}

const fn default_engine_stdout_bytes() -> usize {
    8 * 1024 * 1024
}

const fn default_engine_stderr_bytes() -> usize {
    64 * 1024
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimeConfig {
    pub program: Option<PathBuf>,
    pub required_version: Option<String>,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RendererBundleConfig {
    pub path: Option<PathBuf>,
    pub require_lock_match: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedConfig {
    pub schema_version: u32,
    pub profile: String,
    pub mode: SessionMode,
    pub fallback: FallbackPolicy,
    pub detection: DetectionPolicy,
    pub engines: BTreeMap<String, EngineSelectionPolicy>,
    pub render: RenderPolicy,
    pub presentation: PresentationPolicy,
    pub cache: CachePolicyConfig,
    pub diagnostics: DiagnosticsPolicy,
    pub external_engines: BTreeMap<String, ExternalEngineConfig>,
    pub runtimes: BTreeMap<String, RuntimeConfig>,
    pub renderer_bundle: RendererBundleConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DetectionPolicy {
    pub mode: DetectionMode,
    pub mermaid: bool,
    pub block_math: bool,
    pub max_buffer_bytes: usize,
    pub max_line_bytes: usize,
    pub mermaid_fences: Vec<String>,
    pub math_fences: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EngineSelectionPolicy {
    pub candidates: Vec<String>,
    pub preferred_artifacts: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RenderPolicy {
    pub soft_latency_budget_ms: u64,
    pub hard_timeout_ms: u64,
    pub max_in_flight: usize,
    pub ordering: RenderOrdering,
    pub prewarm: bool,
    pub worker_idle_ms: u64,
    pub worker_max_requests: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PresentationPolicy {
    pub mode: PresentationMode,
    pub prefer: Vec<String>,
    pub image_protocols: Vec<String>,
    pub unsupported: UnsupportedPresentation,
    pub transparent_background: bool,
    pub max_columns: u16,
    pub max_rows: u16,
    pub preserve_aspect_ratio: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CachePolicyConfig {
    pub backend: CacheBackend,
    pub max_entries: usize,
    pub max_bytes: usize,
    pub ttl_seconds: Option<u64>,
    pub path: Option<PathBuf>,
    pub private: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiagnosticsPolicy {
    pub level: DiagnosticLevel,
    pub format: DiagnosticFormat,
    pub sink: DiagnosticSink,
    pub path: Option<PathBuf>,
    pub include_source: bool,
    pub metrics: bool,
}
