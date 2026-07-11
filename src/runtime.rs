use crate::artifact::{ArtifactFormat, RenderArtifact};
use crate::cache::{ArtifactCache, CachePolicy, MemoryArtifactCache, NoopArtifactCache};
use crate::config::{
    CacheBackend, ConfigSnapshot, ConfiguredExecutionModel, ConfiguredLayout, DetectionMode,
    EngineType, PresentationMode, ResolvedConfig, SessionMode,
};
use crate::coordinator::RenderCoordinator;
use crate::detector::{
    FencedDetector, FencedDetectorOptions, PassthroughDetector, SemanticDetector,
};
use crate::engine::{
    EngineDescriptor, EngineRegistry, ExecutionModel, PolicyEngineSelector, RenderEngine,
    RenderRequest,
};
use crate::fingerprint::stable_fingerprint;
use crate::model::BlockKind;
use crate::predisplay::{
    DisplayInterceptor, PreDisplayError, PreDisplayRenderer, PreDisplayReport,
};
use crate::presenter::{
    ArtifactPresenter, SourcePresenter, TerminalCapabilities, TerminalTextPresenter,
};
use crate::process_engine::{ProcessEngine, ProcessEngineConfig, ProgramResolution};
use crate::renderer::{
    BlockRenderer, CoordinatedRenderer, PreviewRenderer, RenderContext, RenderError, SourceRenderer,
};
use crate::terminal::TerminalOutputGate;
use crate::ui::{LayoutSensitivity, Viewport};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeMode {
    #[default]
    Preview,
    Terminal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeRequest {
    pub mode: RuntimeMode,
    pub source_only: bool,
    pub strict: bool,
    pub disable_cache: bool,
    pub color: bool,
    pub viewport: Viewport,
    pub terminal_capabilities: TerminalCapabilities,
    pub theme_fingerprint: u64,
    pub max_buffer_bytes: Option<usize>,
}

impl RuntimeRequest {
    pub const fn preview() -> Self {
        Self {
            mode: RuntimeMode::Preview,
            source_only: false,
            strict: false,
            disable_cache: false,
            color: false,
            viewport: Viewport::cells(80, 24),
            terminal_capabilities: TerminalCapabilities {
                inline_images: false,
                sixel: false,
                color: false,
            },
            theme_fingerprint: 0,
            max_buffer_bytes: None,
        }
    }

    pub const fn terminal(viewport: Viewport, capabilities: TerminalCapabilities) -> Self {
        Self {
            mode: RuntimeMode::Terminal,
            source_only: false,
            strict: false,
            disable_cache: false,
            color: capabilities.color,
            viewport,
            terminal_capabilities: capabilities,
            theme_fingerprint: 0,
            max_buffer_bytes: None,
        }
    }
}

impl Default for RuntimeRequest {
    fn default() -> Self {
        Self::preview()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnavailableEngine {
    pub id: String,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct RuntimeBuildReport {
    pub snapshot_generation: u64,
    pub snapshot_fingerprint: u64,
    pub profile: String,
    pub engine_providers: Vec<String>,
    pub detector_provider: String,
    pub cache_provider: String,
    pub presenter_provider: String,
    pub registered_engines: Vec<EngineDescriptor>,
    pub unavailable_engines: Vec<UnavailableEngine>,
    pub selected_cache_backend: String,
    pub selected_presenter: String,
    pub terminal_capability_fingerprint: u64,
    pub warnings: Vec<String>,
}

impl RuntimeBuildReport {
    fn new(snapshot: &ConfigSnapshot) -> Self {
        Self {
            snapshot_generation: snapshot.generation(),
            snapshot_fingerprint: snapshot.fingerprint(),
            profile: snapshot.config().profile.clone(),
            engine_providers: Vec::new(),
            detector_provider: String::new(),
            cache_provider: String::new(),
            presenter_provider: String::new(),
            registered_engines: Vec::new(),
            unavailable_engines: Vec::new(),
            selected_cache_backend: String::new(),
            selected_presenter: String::new(),
            terminal_capability_fingerprint: 0,
            warnings: Vec::new(),
        }
    }

    pub fn unavailable_engine(&mut self, id: impl Into<String>, reason: impl Into<String>) {
        self.unavailable_engines.push(UnavailableEngine {
            id: id.into(),
            reason: reason.into(),
        });
    }

    pub fn warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    pub fn summary(&self) -> String {
        format!(
            "profile={} generation={} engines={} unavailable={} cache={} presenter={}",
            self.profile,
            self.snapshot_generation,
            self.registered_engines.len(),
            self.unavailable_engines.len(),
            self.selected_cache_backend,
            self.selected_presenter
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBuildError {
    message: String,
}

impl RuntimeBuildError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RuntimeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for RuntimeBuildError {}

impl From<RenderError> for RuntimeBuildError {
    fn from(error: RenderError) -> Self {
        Self::new(error.to_string())
    }
}

pub struct RuntimeBuildContext<'a> {
    pub snapshot: &'a ConfigSnapshot,
    pub request: &'a RuntimeRequest,
}

impl RuntimeBuildContext<'_> {
    pub fn config(&self) -> &ResolvedConfig {
        self.snapshot.config()
    }
}

pub trait EngineProvider: Send + Sync {
    fn id(&self) -> &str;

    fn register(
        &self,
        context: &RuntimeBuildContext<'_>,
        registry: &mut EngineRegistry,
        report: &mut RuntimeBuildReport,
    ) -> Result<(), RuntimeBuildError>;
}

pub trait DetectorProvider: Send + Sync {
    fn id(&self) -> &str;

    fn build(
        &self,
        context: &RuntimeBuildContext<'_>,
    ) -> Result<Box<dyn SemanticDetector>, RuntimeBuildError>;
}

pub trait CacheProvider: Send + Sync {
    fn id(&self) -> &str;

    fn build(
        &self,
        context: &RuntimeBuildContext<'_>,
    ) -> Result<(Box<dyn ArtifactCache>, String), RuntimeBuildError>;
}

pub trait PresenterProvider: Send + Sync {
    fn id(&self) -> &str;

    fn build(
        &self,
        context: &RuntimeBuildContext<'_>,
    ) -> Result<Box<dyn ArtifactPresenter>, RuntimeBuildError>;
}

pub struct RuntimeBuilder {
    engine_providers: Vec<Box<dyn EngineProvider>>,
    engine_provider_ids: BTreeSet<String>,
    detector_provider: Box<dyn DetectorProvider>,
    cache_provider: Box<dyn CacheProvider>,
    presenter_provider: Box<dyn PresenterProvider>,
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        let engine_providers: Vec<Box<dyn EngineProvider>> = vec![
            Box::new(BuiltinEngineProvider),
            Box::new(RendererBundleEngineProvider),
            Box::new(ConfiguredEngineProvider),
        ];
        let engine_provider_ids = engine_providers
            .iter()
            .map(|provider| provider.id().to_owned())
            .collect();
        Self {
            engine_providers,
            engine_provider_ids,
            detector_provider: Box::new(DefaultDetectorProvider),
            cache_provider: Box::new(DefaultCacheProvider),
            presenter_provider: Box::new(DefaultPresenterProvider),
        }
    }
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_engine_provider(
        mut self,
        provider: impl EngineProvider + 'static,
    ) -> Result<Self, RuntimeBuildError> {
        let id = provider.id().to_owned();
        if id.trim().is_empty() {
            return Err(RuntimeBuildError::new("engine provider ID cannot be empty"));
        }
        if !self.engine_provider_ids.insert(id.clone()) {
            return Err(RuntimeBuildError::new(format!(
                "engine provider `{id}` is already registered"
            )));
        }
        self.engine_providers.push(Box::new(provider));
        Ok(self)
    }

    pub fn with_detector_provider(mut self, provider: impl DetectorProvider + 'static) -> Self {
        self.detector_provider = Box::new(provider);
        self
    }

    pub fn with_cache_provider(mut self, provider: impl CacheProvider + 'static) -> Self {
        self.cache_provider = Box::new(provider);
        self
    }

    pub fn with_presenter_provider(mut self, provider: impl PresenterProvider + 'static) -> Self {
        self.presenter_provider = Box::new(provider);
        self
    }

    pub fn build(
        self,
        snapshot: ConfigSnapshot,
        request: RuntimeRequest,
    ) -> Result<SessionRuntime, RuntimeBuildError> {
        let context = RuntimeBuildContext {
            snapshot: &snapshot,
            request: &request,
        };
        let mut report = RuntimeBuildReport::new(&snapshot);
        report.detector_provider = self.detector_provider.id().to_owned();
        report.cache_provider = self.cache_provider.id().to_owned();
        report.presenter_provider = self.presenter_provider.id().to_owned();
        report.terminal_capability_fingerprint = request.terminal_capabilities.fingerprint();

        let detector = self.detector_provider.build(&context)?;
        let (cache, cache_name) = self.cache_provider.build(&context)?;
        report.selected_cache_backend = cache_name;
        let presenter = self.presenter_provider.build(&context)?;
        report.selected_presenter = presenter.id().to_owned();

        let mut registry = EngineRegistry::new();
        for provider in &self.engine_providers {
            report.engine_providers.push(provider.id().to_owned());
            provider.register(&context, &mut registry, &mut report)?;
        }
        report.registered_engines = registry.descriptors();

        let selector = selector_for(&context);
        let coordinator = RenderCoordinator::with_boxed_cache(registry, selector, cache);
        let coordinated =
            CoordinatedRenderer::new(coordinator, presenter, request.terminal_capabilities);
        let renderer: Box<dyn BlockRenderer> = Box::new(coordinated);

        let option_material = format!(
            "{}:{:?}:{}:{}:{}:{:?}:{}:{:?}",
            snapshot.fingerprint(),
            request.mode,
            request.source_only,
            request.strict,
            request.color,
            request.viewport,
            request.theme_fingerprint,
            request.max_buffer_bytes,
        );
        let context = RenderContext {
            color: request.color,
            terminal_width: Some(usize::from(request.viewport.columns)),
            theme_fingerprint: request.theme_fingerprint,
            options_fingerprint: stable_fingerprint(option_material.as_bytes()),
        };
        let strict =
            request.strict || snapshot.config().fallback == crate::config::FallbackPolicy::Error;
        let pre_display = PreDisplayRenderer::new(detector, renderer)
            .with_context(context)
            .strict(strict);
        let interceptor = DisplayInterceptor::new(TerminalOutputGate::default(), pre_display);

        Ok(SessionRuntime {
            snapshot,
            build_report: report,
            interceptor,
        })
    }
}

pub struct SessionRuntime {
    snapshot: ConfigSnapshot,
    build_report: RuntimeBuildReport,
    interceptor:
        DisplayInterceptor<TerminalOutputGate, Box<dyn SemanticDetector>, Box<dyn BlockRenderer>>,
}

impl SessionRuntime {
    pub fn snapshot(&self) -> &ConfigSnapshot {
        &self.snapshot
    }

    pub fn build_report(&self) -> &RuntimeBuildReport {
        &self.build_report
    }

    pub fn processing_report(&self) -> &PreDisplayReport {
        self.interceptor.report()
    }

    pub fn feed(&mut self, input: &[u8], display: &mut dyn Write) -> Result<(), PreDisplayError> {
        self.interceptor.feed(input, display)
    }

    pub fn finish(&mut self, display: &mut dyn Write) -> Result<(), PreDisplayError> {
        self.interceptor.finish(display)
    }
}

#[derive(Debug)]
struct RendererEngine<R> {
    descriptor: EngineDescriptor,
    renderer: R,
    format: ArtifactFormat,
}

impl<R> RendererEngine<R> {
    fn new(descriptor: EngineDescriptor, renderer: R, format: ArtifactFormat) -> Self {
        Self {
            descriptor,
            renderer,
            format,
        }
    }
}

impl<R: BlockRenderer> RenderEngine for RendererEngine<R> {
    fn descriptor(&self) -> &EngineDescriptor {
        &self.descriptor
    }

    fn render(&mut self, request: &RenderRequest<'_>) -> Result<RenderArtifact, RenderError> {
        Ok(RenderArtifact::new(
            self.format,
            self.renderer.render(request.block, request.context)?,
            self.descriptor.identity.clone(),
            request.block.kind(),
            self.descriptor.layout_sensitivity,
        ))
    }
}

#[derive(Clone, Copy, Debug)]
struct BuiltinEngineProvider;

impl EngineProvider for BuiltinEngineProvider {
    fn id(&self) -> &str {
        "builtin/core-engines-v1"
    }

    fn register(
        &self,
        _context: &RuntimeBuildContext<'_>,
        registry: &mut EngineRegistry,
        _report: &mut RuntimeBuildReport,
    ) -> Result<(), RuntimeBuildError> {
        registry.register(RendererEngine::new(
            EngineDescriptor::new(
                "preview",
                env!("CARGO_PKG_VERSION"),
                vec![BlockKind::Math, BlockKind::Mermaid],
                vec![ArtifactFormat::TerminalText],
                LayoutSensitivity::Columns,
                ExecutionModel::InProcess,
            ),
            PreviewRenderer,
            ArtifactFormat::TerminalText,
        ))?;
        registry.register(RendererEngine::new(
            EngineDescriptor::new(
                "source",
                env!("CARGO_PKG_VERSION"),
                vec![BlockKind::Math, BlockKind::Mermaid],
                vec![ArtifactFormat::Source],
                LayoutSensitivity::Independent,
                ExecutionModel::InProcess,
            ),
            SourceRenderer,
            ArtifactFormat::Source,
        ))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct RendererBundleEngineProvider;

impl EngineProvider for RendererBundleEngineProvider {
    fn id(&self) -> &str {
        "builtin/renderer-bundle-v1"
    }

    fn register(
        &self,
        context: &RuntimeBuildContext<'_>,
        registry: &mut EngineRegistry,
        report: &mut RuntimeBuildReport,
    ) -> Result<(), RuntimeBuildError> {
        let Some(bundle) = renderer_bundle_path(context.config()) else {
            for id in ["mermaid-worker", "mermaid-cli", "mathjax-worker", "katex"] {
                report.unavailable_engine(id, "renderer bundle was not configured or discovered");
            }
            return Ok(());
        };
        let bundle = std::fs::canonicalize(&bundle).unwrap_or(bundle);
        let worker = bundle.join("worker.mjs");
        if !worker.is_file() {
            for id in ["mermaid-worker", "mermaid-cli", "mathjax-worker", "katex"] {
                report.unavailable_engine(
                    id,
                    format!("renderer worker is missing: {}", worker.display()),
                );
            }
            return Ok(());
        }

        let node = context
            .config()
            .runtimes
            .get("node")
            .and_then(|runtime| runtime.program.clone())
            .unwrap_or_else(|| PathBuf::from("node"));
        let resolution = if node.is_absolute() {
            ProgramResolution::AbsoluteOnly
        } else {
            ProgramResolution::PathSearch
        };
        let timeout = Duration::from_millis(context.config().render.hard_timeout_ms);

        register_bundle_engine(
            registry,
            BundleEngineSpec {
                id: "mermaid-worker",
                version: "11.16.0-stdio-v1",
                kind: BlockKind::Mermaid,
                format: ArtifactFormat::Svg,
                layout: LayoutSensitivity::Pixels,
                variant: "mermaid",
            },
            &node,
            resolution,
            &worker,
            &bundle,
            timeout,
        )?;
        register_bundle_engine(
            registry,
            BundleEngineSpec {
                id: "mermaid-cli",
                version: "11.16.0-stdio-v1",
                kind: BlockKind::Mermaid,
                format: ArtifactFormat::Svg,
                layout: LayoutSensitivity::Pixels,
                variant: "mermaid",
            },
            &node,
            resolution,
            &worker,
            &bundle,
            timeout,
        )?;
        register_bundle_engine(
            registry,
            BundleEngineSpec {
                id: "mathjax-worker",
                version: "4.1.3-stdio-v1",
                kind: BlockKind::Math,
                format: ArtifactFormat::Svg,
                layout: LayoutSensitivity::Columns,
                variant: "mathjax",
            },
            &node,
            resolution,
            &worker,
            &bundle,
            timeout,
        )?;
        register_bundle_engine(
            registry,
            BundleEngineSpec {
                id: "katex",
                version: "0.17.0-stdio-v1",
                kind: BlockKind::Math,
                format: ArtifactFormat::MathMl,
                layout: LayoutSensitivity::Columns,
                variant: "katex",
            },
            &node,
            resolution,
            &worker,
            &bundle,
            timeout,
        )?;
        report.warning(
            "renderer-bundle worker roles currently use bounded one-shot stdio transport; persistent transport remains a replaceable follow-up provider",
        );
        Ok(())
    }
}

struct BundleEngineSpec {
    id: &'static str,
    version: &'static str,
    kind: BlockKind,
    format: ArtifactFormat,
    layout: LayoutSensitivity,
    variant: &'static str,
}

#[allow(clippy::too_many_arguments)]
fn register_bundle_engine(
    registry: &mut EngineRegistry,
    spec: BundleEngineSpec,
    node: &Path,
    resolution: ProgramResolution,
    worker: &Path,
    bundle: &Path,
    timeout: Duration,
) -> Result<(), RuntimeBuildError> {
    let mut environment = BTreeMap::new();
    environment.insert(
        OsString::from("PTYMARK_ENGINE_VARIANT"),
        OsString::from(spec.variant),
    );
    let config = ProcessEngineConfig {
        id: spec.id.to_owned(),
        version: spec.version.to_owned(),
        supported_kinds: vec![spec.kind],
        formats: vec![spec.format],
        layout_sensitivity: spec.layout,
        execution_model: ExecutionModel::OneShotProcess,
        program: node.to_path_buf(),
        program_resolution: resolution,
        arguments: vec![worker.as_os_str().to_owned(), OsString::from("--stdio-v1")],
        timeout,
        max_stdout_bytes: 8 * 1024 * 1024,
        max_stderr_bytes: 64 * 1024,
        working_directory: Some(bundle.to_path_buf()),
        environment,
        inherit_environment: [
            "PATH",
            "LANG",
            "LC_ALL",
            "HOME",
            "TMPDIR",
            "XDG_CACHE_HOME",
            "FONTCONFIG_PATH",
            "FONTCONFIG_FILE",
            "PUPPETEER_EXECUTABLE_PATH",
        ]
        .into_iter()
        .map(OsString::from)
        .collect(),
    };
    registry.register(ProcessEngine::new(config)?)?;
    Ok(())
}

fn renderer_bundle_path(config: &ResolvedConfig) -> Option<PathBuf> {
    config
        .renderer_bundle
        .path
        .clone()
        .or_else(|| std::env::var_os("PTYMARK_RENDERER_ROOT").map(PathBuf::from))
        .or_else(|| {
            let candidate = PathBuf::from("/opt/ptymark-renderers");
            candidate.is_dir().then_some(candidate)
        })
}

#[derive(Clone, Copy, Debug)]
struct ConfiguredEngineProvider;

impl EngineProvider for ConfiguredEngineProvider {
    fn id(&self) -> &str {
        "configured/process-engines-v1"
    }

    fn register(
        &self,
        context: &RuntimeBuildContext<'_>,
        registry: &mut EngineRegistry,
        _report: &mut RuntimeBuildReport,
    ) -> Result<(), RuntimeBuildError> {
        for (id, engine) in &context.config().external_engines {
            if engine.engine_type != EngineType::Process {
                return Err(RuntimeBuildError::new(format!(
                    "configured engine `{id}` has an unsupported engine type"
                )));
            }
            if engine.execution == ConfiguredExecutionModel::PersistentWorker {
                return Err(RuntimeBuildError::new(format!(
                    "configured engine `{id}` requests persistent-worker execution, which requires a dedicated worker provider"
                )));
            }
            let supported_kinds = engine
                .semantic_kinds
                .iter()
                .map(|kind| parse_kind(kind))
                .collect::<Result<Vec<_>, _>>()?;
            let formats = engine
                .artifact_types
                .iter()
                .map(|format| parse_artifact_format(format))
                .collect::<Result<Vec<_>, _>>()?;
            let layout_sensitivity = match engine.layout {
                ConfiguredLayout::Independent => LayoutSensitivity::Independent,
                ConfiguredLayout::Columns => LayoutSensitivity::Columns,
                ConfiguredLayout::Pixels => LayoutSensitivity::Pixels,
                ConfiguredLayout::FullViewport => LayoutSensitivity::FullViewport,
            };
            let environment = engine
                .environment
                .iter()
                .map(|(key, value)| (OsString::from(key), OsString::from(value)))
                .collect();
            let config = ProcessEngineConfig {
                id: id.clone(),
                version: engine.version.clone(),
                supported_kinds,
                formats,
                layout_sensitivity,
                execution_model: ExecutionModel::OneShotProcess,
                program: engine.program.clone(),
                program_resolution: ProgramResolution::AbsoluteOnly,
                arguments: engine.args.iter().map(OsString::from).collect(),
                timeout: Duration::from_millis(engine.timeout_ms),
                max_stdout_bytes: engine.max_stdout_bytes,
                max_stderr_bytes: engine.max_stderr_bytes,
                working_directory: engine.working_directory.clone(),
                environment,
                inherit_environment: engine
                    .inherit_environment
                    .iter()
                    .map(OsString::from)
                    .collect(),
            };
            registry.register(ProcessEngine::new(config)?)?;
        }
        Ok(())
    }
}

fn parse_kind(kind: &str) -> Result<BlockKind, RuntimeBuildError> {
    match kind {
        "math" => Ok(BlockKind::Math),
        "mermaid" => Ok(BlockKind::Mermaid),
        _ => Err(RuntimeBuildError::new(format!(
            "unknown configured semantic kind `{kind}`"
        ))),
    }
}

fn parse_artifact_format(format: &str) -> Result<ArtifactFormat, RuntimeBuildError> {
    match format {
        "image/svg+xml" => Ok(ArtifactFormat::Svg),
        "image/png" => Ok(ArtifactFormat::Png),
        "application/mathml+xml" => Ok(ArtifactFormat::MathMl),
        "text/plain" | "text/plain; charset=utf-8" => Ok(ArtifactFormat::TerminalText),
        _ => Err(RuntimeBuildError::new(format!(
            "unknown configured artifact type `{format}`"
        ))),
    }
}

#[derive(Clone, Copy, Debug)]
struct DefaultDetectorProvider;

impl DetectorProvider for DefaultDetectorProvider {
    fn id(&self) -> &str {
        "builtin/explicit-detector-v1"
    }

    fn build(
        &self,
        context: &RuntimeBuildContext<'_>,
    ) -> Result<Box<dyn SemanticDetector>, RuntimeBuildError> {
        let config = context.config();
        if config.mode == SessionMode::Bypass || config.detection.mode == DetectionMode::Off {
            return Ok(Box::new(PassthroughDetector));
        }
        Ok(Box::new(FencedDetector::with_options(
            FencedDetectorOptions {
                max_buffer_bytes: context
                    .request
                    .max_buffer_bytes
                    .unwrap_or(config.detection.max_buffer_bytes),
                max_line_bytes: config.detection.max_line_bytes,
                mermaid: config.detection.mermaid,
                block_math: config.detection.block_math,
                mermaid_fences: config.detection.mermaid_fences.clone(),
                math_fences: config.detection.math_fences.clone(),
            },
        )))
    }
}

#[derive(Clone, Copy, Debug)]
struct DefaultCacheProvider;

impl CacheProvider for DefaultCacheProvider {
    fn id(&self) -> &str {
        "builtin/cache-v1"
    }

    fn build(
        &self,
        context: &RuntimeBuildContext<'_>,
    ) -> Result<(Box<dyn ArtifactCache>, String), RuntimeBuildError> {
        let cache = &context.config().cache;
        if context.request.disable_cache || cache.private || cache.backend == CacheBackend::None {
            return Ok((Box::new(NoopArtifactCache::default()), "none".to_owned()));
        }
        match cache.backend {
            CacheBackend::Memory => Ok((
                Box::new(MemoryArtifactCache::new(CachePolicy::new(
                    cache.max_entries,
                    cache.max_bytes,
                ))),
                "memory".to_owned(),
            )),
            CacheBackend::Disk | CacheBackend::Tiered => Err(RuntimeBuildError::new(format!(
                "cache backend `{:?}` is configured but no persistent cache provider is registered",
                cache.backend
            ))),
            CacheBackend::None => unreachable!("handled above"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct DefaultPresenterProvider;

impl PresenterProvider for DefaultPresenterProvider {
    fn id(&self) -> &str {
        "builtin/presenter-v1"
    }

    fn build(
        &self,
        context: &RuntimeBuildContext<'_>,
    ) -> Result<Box<dyn ArtifactPresenter>, RuntimeBuildError> {
        let config = context.config();
        if context.request.source_only
            || config.mode == SessionMode::Source
            || config.presentation.mode == PresentationMode::Source
        {
            return Ok(Box::new(SourcePresenter::default()));
        }
        Ok(Box::new(TerminalTextPresenter::default()))
    }
}

fn selector_for(context: &RuntimeBuildContext<'_>) -> PolicyEngineSelector {
    let source_only = context.request.source_only
        || context.config().mode == SessionMode::Source
        || context.config().presentation.mode == PresentationMode::Source;
    let mut selector = PolicyEngineSelector::new();
    for (kind, name) in [(BlockKind::Math, "math"), (BlockKind::Mermaid, "mermaid")] {
        if source_only {
            selector.set_candidates(kind, ["source"]);
            continue;
        }
        let mut candidates = context
            .config()
            .engines
            .get(name)
            .map(|policy| policy.candidates.clone())
            .unwrap_or_else(|| vec!["source".to_owned()]);
        remove_candidate(&mut candidates, "source");
        if context.request.mode == RuntimeMode::Preview {
            remove_candidate(&mut candidates, "preview");
            candidates.insert(0, "preview".to_owned());
        }
        candidates.push("source".to_owned());
        selector.set_candidates(kind, candidates);
    }
    selector
}

fn remove_candidate(candidates: &mut Vec<String>, id: &str) {
    candidates.retain(|candidate| candidate != id);
}
