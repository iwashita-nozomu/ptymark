use ptymark::{
    ArtifactFormat, BlockKind, CacheDisposition, CachePolicy, EngineDescriptor, EngineRegistry,
    ExecutionModel, LayoutSensitivity, MemoryArtifactCache, NoopArtifactCache,
    PolicyEngineSelector, RenderArtifact, RenderContext, RenderCoordinator, RenderEngine,
    RenderError, RenderRequest, SemanticBlock,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct FakeEngine {
    descriptor: EngineDescriptor,
    calls: Arc<AtomicUsize>,
    failure: Option<&'static str>,
    payload: Vec<u8>,
}

impl FakeEngine {
    fn new(
        id: &str,
        calls: Arc<AtomicUsize>,
        failure: Option<&'static str>,
        payload: &[u8],
    ) -> Self {
        Self {
            descriptor: EngineDescriptor::new(
                id,
                "1",
                vec![BlockKind::Math],
                vec![ArtifactFormat::Svg],
                LayoutSensitivity::Columns,
                ExecutionModel::InProcess,
            ),
            calls,
            failure,
            payload: payload.to_vec(),
        }
    }
}

impl RenderEngine for FakeEngine {
    fn descriptor(&self) -> &EngineDescriptor {
        &self.descriptor
    }

    fn render(&mut self, request: &RenderRequest<'_>) -> Result<RenderArtifact, RenderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(message) = self.failure {
            return Err(RenderError::new(message));
        }
        Ok(RenderArtifact::new(
            request.preferred_format,
            self.payload.clone(),
            self.descriptor.identity.clone(),
            request.block.kind(),
            self.descriptor.layout_sensitivity,
        ))
    }
}

fn math() -> SemanticBlock {
    SemanticBlock::new(
        BlockKind::Math,
        b"$$\nE = mc^2\n$$\n".to_vec(),
        b"E = mc^2\n".to_vec(),
    )
}

#[test]
fn coordinator_selects_fallback_engine_then_caches_success() {
    let primary_calls = Arc::new(AtomicUsize::new(0));
    let fallback_calls = Arc::new(AtomicUsize::new(0));
    let mut registry = EngineRegistry::new();
    registry
        .register(FakeEngine::new(
            "math/primary",
            primary_calls.clone(),
            Some("primary failed"),
            b"",
        ))
        .expect("register primary");
    registry
        .register(FakeEngine::new(
            "math/fallback",
            fallback_calls.clone(),
            None,
            b"<svg/>",
        ))
        .expect("register fallback");
    let selector = PolicyEngineSelector::new()
        .with_candidates(BlockKind::Math, ["math/primary", "math/fallback"]);
    let mut coordinator = RenderCoordinator::new(
        registry,
        selector,
        MemoryArtifactCache::new(CachePolicy::default()),
    );
    let context = RenderContext {
        terminal_width: Some(80),
        ..RenderContext::default()
    };

    let first = coordinator
        .render(&math(), &context, &[ArtifactFormat::Svg], "test/svg", 1)
        .expect("first render");
    assert_eq!(first.cache, CacheDisposition::MissStored);
    assert_eq!(first.attempts.len(), 2);
    assert_eq!(primary_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fallback_calls.load(Ordering::SeqCst), 1);

    let second = coordinator
        .render(&math(), &context, &[ArtifactFormat::Svg], "test/svg", 1)
        .expect("cached render");
    assert_eq!(second.cache, CacheDisposition::Hit);
    assert_eq!(primary_calls.load(Ordering::SeqCst), 2);
    assert_eq!(fallback_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn noop_cache_keeps_engine_selection_unchanged() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = EngineRegistry::new();
    registry
        .register(FakeEngine::new("math/only", calls.clone(), None, b"<svg/>"))
        .expect("register");
    let selector = PolicyEngineSelector::new().with_candidates(BlockKind::Math, ["math/only"]);
    let mut coordinator = RenderCoordinator::new(registry, selector, NoopArtifactCache::default());

    for _ in 0..2 {
        coordinator
            .render(
                &math(),
                &RenderContext::default(),
                &[ArtifactFormat::Svg],
                "test/svg",
                1,
            )
            .expect("render");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(coordinator.cache_stats().entries, 0);
}

#[test]
fn presenter_format_is_part_of_engine_eligibility() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = EngineRegistry::new();
    registry
        .register(FakeEngine::new("math/svg", calls, None, b"<svg/>"))
        .expect("register");
    let selector = PolicyEngineSelector::new().with_candidates(BlockKind::Math, ["math/svg"]);
    let mut coordinator = RenderCoordinator::new(
        registry,
        selector,
        MemoryArtifactCache::new(CachePolicy::default()),
    );
    let error = coordinator
        .render(
            &math(),
            &RenderContext::default(),
            &[ArtifactFormat::TerminalText],
            "terminal/text",
            0,
        )
        .expect_err("format mismatch must fail");
    assert!(error.to_string().contains("no format accepted"));
}
