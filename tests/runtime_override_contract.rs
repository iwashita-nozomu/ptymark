use ptymark::{
    ArtifactFormat, BlockKind, ConfigEnvironment, ConfigManager, ConfigRequest, EngineDescriptor,
    EngineProvider, EngineRegistry, ExecutionModel, LayoutSensitivity, RenderArtifact, RenderEngine,
    RenderError, RenderRequest, RuntimeBuildContext, RuntimeBuildError, RuntimeBuildReport,
    RuntimeBuilder, RuntimeRequest,
};

struct ReplacementPreview {
    descriptor: EngineDescriptor,
}

impl ReplacementPreview {
    fn new() -> Self {
        Self {
            descriptor: EngineDescriptor::new(
                "preview",
                "replacement-1",
                vec![BlockKind::Math, BlockKind::Mermaid],
                vec![ArtifactFormat::TerminalText],
                LayoutSensitivity::Independent,
                ExecutionModel::InProcess,
            ),
        }
    }
}

impl RenderEngine for ReplacementPreview {
    fn descriptor(&self) -> &EngineDescriptor {
        &self.descriptor
    }

    fn render(&mut self, request: &RenderRequest<'_>) -> Result<RenderArtifact, RenderError> {
        Ok(RenderArtifact::new(
            ArtifactFormat::TerminalText,
            format!("replacement:{}\n", request.block.kind()).into_bytes(),
            self.descriptor.identity.clone(),
            request.block.kind(),
            self.descriptor.layout_sensitivity,
        ))
    }
}

struct ReplacementProvider;

impl EngineProvider for ReplacementProvider {
    fn id(&self) -> &str {
        "example/replacement-provider"
    }

    fn register(
        &self,
        _context: &RuntimeBuildContext<'_>,
        registry: &mut EngineRegistry,
        _report: &mut RuntimeBuildReport,
    ) -> Result<(), RuntimeBuildError> {
        registry.register(ReplacementPreview::new())?;
        Ok(())
    }
}

fn snapshot() -> ptymark::ConfigSnapshot {
    ConfigManager::default()
        .load(ConfigRequest::default(), ConfigEnvironment::default())
        .expect("load built-ins")
        .into_snapshot(1)
        .expect("snapshot")
}

#[test]
fn replacing_the_builtin_catalog_requires_an_explicit_opt_out() {
    let duplicate = RuntimeBuilder::default()
        .with_engine_provider(ReplacementProvider)
        .expect("provider ID is unique")
        .build(snapshot(), RuntimeRequest::preview())
        .err()
        .expect("engine ID collision must fail");
    assert!(duplicate.to_string().contains("already registered"));

    let mut runtime = RuntimeBuilder::default()
        .without_engine_providers()
        .with_engine_provider(ReplacementProvider)
        .expect("register replacement provider")
        .build(snapshot(), RuntimeRequest::preview())
        .expect("explicit replacement runtime");
    let mut display = Vec::new();
    runtime
        .feed(b"$$\nE = mc^2\n$$\n", &mut display)
        .expect("feed");
    runtime.finish(&mut display).expect("finish");
    assert_eq!(display, b"replacement:math\n");
    assert_eq!(runtime.build_report().registered_engines.len(), 1);
    assert_eq!(
        runtime.build_report().registered_engines[0].identity.version,
        "replacement-1"
    );
}
