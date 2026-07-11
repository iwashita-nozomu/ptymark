use ptymark::{
    ArtifactFormat, BlockKind, ConfigEnvironment, ConfigManager, ConfigRequest, EngineDescriptor,
    EngineProvider, EngineRegistry, ExecutionModel, LayoutSensitivity, RenderArtifact, RenderEngine,
    RenderError, RenderRequest, RuntimeBuildContext, RuntimeBuildError, RuntimeBuildReport,
    RuntimeBuilder, RuntimeRequest,
};

fn snapshot(profile: Option<&str>) -> ptymark::ConfigSnapshot {
    ConfigManager::default()
        .load(
            ConfigRequest {
                profile: profile.map(str::to_owned),
                ..ConfigRequest::default()
            },
            ConfigEnvironment::default(),
        )
        .expect("load configuration")
        .into_snapshot(1)
        .expect("freeze configuration")
}

#[test]
fn runtime_builder_composes_the_preview_pipeline() {
    let mut runtime = RuntimeBuilder::default()
        .build(snapshot(None), RuntimeRequest::preview())
        .expect("build runtime");
    let mut display = Vec::new();
    runtime
        .feed(b"before\n$$\nE = mc^2\n$$\nafter\n", &mut display)
        .expect("feed");
    runtime.finish(&mut display).expect("finish");

    let text = String::from_utf8(display).expect("UTF-8 preview");
    assert!(text.starts_with("before\n"));
    assert!(text.contains("ptymark math preview"));
    assert!(text.ends_with("after\n"));
    assert_eq!(runtime.processing_report().rendered_blocks, 1);
    assert!(runtime
        .build_report()
        .registered_engines
        .iter()
        .any(|descriptor| descriptor.identity.id == "preview"));
    assert!(runtime
        .build_report()
        .registered_engines
        .iter()
        .any(|descriptor| descriptor.identity.id == "source"));
}

#[test]
fn source_only_runtime_is_exact_and_uses_no_cache() {
    let mut request = RuntimeRequest::preview();
    request.source_only = true;
    let mut runtime = RuntimeBuilder::default()
        .build(snapshot(None), request)
        .expect("build runtime");
    let source = b"before\n```mermaid\nA --> B\n```\nafter\n";
    let mut display = Vec::new();
    runtime.feed(source, &mut display).expect("feed");
    runtime.finish(&mut display).expect("finish");

    assert_eq!(display, source);
    assert_eq!(runtime.build_report().selected_presenter, "terminal/source-v1");
}

#[test]
fn private_profile_selects_the_noop_cache_provider() {
    let runtime = RuntimeBuilder::default()
        .build(snapshot(Some("private")), RuntimeRequest::preview())
        .expect("build private runtime");
    assert_eq!(runtime.build_report().selected_cache_backend, "none");
    assert!(runtime.snapshot().config().cache.private);
}

struct ExtensionEngine {
    descriptor: EngineDescriptor,
}

impl ExtensionEngine {
    fn new() -> Self {
        Self {
            descriptor: EngineDescriptor::new(
                "example/runtime-test",
                "1",
                vec![BlockKind::Math],
                vec![ArtifactFormat::TerminalText],
                LayoutSensitivity::Independent,
                ExecutionModel::InProcess,
            ),
        }
    }
}

impl RenderEngine for ExtensionEngine {
    fn descriptor(&self) -> &EngineDescriptor {
        &self.descriptor
    }

    fn render(&mut self, request: &RenderRequest<'_>) -> Result<RenderArtifact, RenderError> {
        Ok(RenderArtifact::new(
            ArtifactFormat::TerminalText,
            request.block.body().to_vec(),
            self.descriptor.identity.clone(),
            request.block.kind(),
            self.descriptor.layout_sensitivity,
        ))
    }
}

#[derive(Clone, Copy)]
struct ExtensionProvider;

impl EngineProvider for ExtensionProvider {
    fn id(&self) -> &str {
        "example/provider"
    }

    fn register(
        &self,
        _context: &RuntimeBuildContext<'_>,
        registry: &mut EngineRegistry,
        _report: &mut RuntimeBuildReport,
    ) -> Result<(), RuntimeBuildError> {
        registry.register(ExtensionEngine::new())?;
        Ok(())
    }
}

#[test]
fn embedding_app_can_register_an_engine_without_changing_the_cli_or_coordinator() {
    let runtime = RuntimeBuilder::default()
        .with_engine_provider(ExtensionProvider)
        .expect("register provider")
        .build(snapshot(None), RuntimeRequest::preview())
        .expect("build runtime");
    assert!(runtime
        .build_report()
        .registered_engines
        .iter()
        .any(|descriptor| descriptor.identity.id == "example/runtime-test"));
    assert!(runtime
        .build_report()
        .engine_providers
        .iter()
        .any(|provider| provider == "example/provider"));
}

#[test]
fn duplicate_provider_identity_is_rejected() {
    let builder = RuntimeBuilder::default()
        .with_engine_provider(ExtensionProvider)
        .expect("first provider");
    let error = builder
        .with_engine_provider(ExtensionProvider)
        .err()
        .expect("duplicate must fail");
    assert!(error.to_string().contains("already registered"));
}
