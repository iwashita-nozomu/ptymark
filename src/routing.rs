use crate::config::{EnginesConfig, MathEngine, MermaidEngine};
use crate::engine::ConfiguredRenderer;
use crate::model::{BlockKind, SemanticBlock};
use crate::render::{
    PreviewRenderer, RenderArtifact, RenderContext, RenderError, Renderer, SourceRenderer,
};

/// Stable logical destinations produced by a render-decision policy.
///
/// The enum is intentionally small. A new route is added only with a concrete
/// implementation, fallback behavior, and contract tests.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RenderRoute {
    Preview,
    Source,
    ConfiguredEngine,
}

impl RenderRoute {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Source => "source",
            Self::ConfiguredEngine => "configured-engine",
        }
    }
}

/// Input available to a policy before an engine is invoked.
///
/// The request contains semantic and display context only. It has no access to
/// terminal input, PTY state, signals, or raw control-sequence output.
#[derive(Clone, Copy, Debug)]
pub struct DecisionRequest<'a> {
    block: &'a SemanticBlock,
    context: RenderContext,
}

impl<'a> DecisionRequest<'a> {
    pub const fn new(block: &'a SemanticBlock, context: RenderContext) -> Self {
        Self { block, context }
    }

    pub const fn block(&self) -> &'a SemanticBlock {
        self.block
    }

    pub const fn context(&self) -> RenderContext {
        self.context
    }
}

/// Immutable result of the decision stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderDecision {
    route: RenderRoute,
}

impl RenderDecision {
    pub const fn new(route: RenderRoute) -> Self {
        Self { route }
    }

    pub const fn route(self) -> RenderRoute {
        self.route
    }
}

/// Chooses how a recognized semantic block should be rendered.
///
/// Implementations should be deterministic and side-effect free. Executable
/// discovery, process creation, and artifact presentation belong to the handoff
/// stage instead.
pub trait RenderDecider: Send {
    fn id(&self) -> &str;

    fn decide(&self, request: DecisionRequest<'_>) -> Result<RenderDecision, RenderError>;
}

/// Decision policy derived from the current concrete engine slots.
#[derive(Clone, Debug)]
pub struct ConfiguredDecider {
    mermaid: RenderRoute,
    math: RenderRoute,
    id: String,
}

impl ConfiguredDecider {
    pub fn new(config: &EnginesConfig) -> Self {
        let mermaid = match config.mermaid.backend {
            MermaidEngine::Preview => RenderRoute::Preview,
            MermaidEngine::Source => RenderRoute::Source,
            MermaidEngine::MermaidCli => RenderRoute::ConfiguredEngine,
        };
        let math = match config.math.backend {
            MathEngine::Preview => RenderRoute::Preview,
            MathEngine::Source => RenderRoute::Source,
            MathEngine::MathjaxCli => RenderRoute::ConfiguredEngine,
        };
        let id = format!(
            "configured-decision-v1;mermaid={};math={}",
            config.mermaid.backend.as_str(),
            config.math.backend.as_str()
        );
        Self {
            mermaid,
            math,
            id,
        }
    }
}

impl RenderDecider for ConfiguredDecider {
    fn id(&self) -> &str {
        &self.id
    }

    fn decide(&self, request: DecisionRequest<'_>) -> Result<RenderDecision, RenderError> {
        let route = match request.block().kind() {
            BlockKind::Mermaid => self.mermaid,
            BlockKind::Math => self.math,
        };
        Ok(RenderDecision::new(route))
    }
}

/// Typed handoff from a decision policy to a rendering implementation.
///
/// Exact source and body bytes remain available independently so a future
/// adapter can choose its protocol without reparsing terminal output.
#[derive(Clone, Copy, Debug)]
pub struct EngineRequest<'a> {
    decision: RenderDecision,
    block: &'a SemanticBlock,
    context: RenderContext,
}

impl<'a> EngineRequest<'a> {
    pub const fn new(
        decision: RenderDecision,
        block: &'a SemanticBlock,
        context: RenderContext,
    ) -> Self {
        Self {
            decision,
            block,
            context,
        }
    }

    pub const fn decision(&self) -> RenderDecision {
        self.decision
    }

    pub const fn block(&self) -> &'a SemanticBlock {
        self.block
    }

    pub const fn context(&self) -> RenderContext {
        self.context
    }
}

/// Output metadata returned by the handoff stage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineResponse {
    pub engine_id: String,
    pub artifact: RenderArtifact,
}

impl EngineResponse {
    pub fn new(engine_id: impl Into<String>, artifact: RenderArtifact) -> Self {
        Self {
            engine_id: engine_id.into(),
            artifact,
        }
    }
}

/// Executes a previously selected route.
///
/// This is the extension boundary for a future persistent worker, in-process
/// engine, remote service, or typed artifact pipeline. It receives no raw
/// terminal stream and must never write directly to stdout.
pub trait EngineHandoff: Send {
    fn id(&self) -> &str;

    fn execute(&mut self, request: EngineRequest<'_>) -> Result<EngineResponse, RenderError>;
}

/// Handoff for the built-in renderers and the currently configured installed
/// engine adapters.
pub struct ConfiguredHandoff {
    preview: PreviewRenderer,
    source: SourceRenderer,
    configured: ConfiguredRenderer,
    id: String,
}

impl ConfiguredHandoff {
    pub fn new(config: &EnginesConfig) -> Self {
        let configured = ConfiguredRenderer::new(config);
        let id = format!("configured-handoff-v1;{}", configured.id());
        Self {
            preview: PreviewRenderer,
            source: SourceRenderer,
            configured,
            id,
        }
    }
}

impl EngineHandoff for ConfiguredHandoff {
    fn id(&self) -> &str {
        &self.id
    }

    fn execute(&mut self, request: EngineRequest<'_>) -> Result<EngineResponse, RenderError> {
        let block = request.block();
        let context = request.context();
        match request.decision().route() {
            RenderRoute::Preview => {
                let artifact = self.preview.render(block, context)?;
                Ok(EngineResponse::new(self.preview.id(), artifact))
            }
            RenderRoute::Source => {
                let artifact = self.source.render(block, context)?;
                Ok(EngineResponse::new(self.source.id(), artifact))
            }
            RenderRoute::ConfiguredEngine => {
                let artifact = self.configured.render(block, context)?;
                Ok(EngineResponse::new(self.configured.id(), artifact))
            }
        }
    }
}

/// Renderer composition that separates policy from engine invocation.
pub struct RoutedRenderer {
    decider: Box<dyn RenderDecider>,
    handoff: Box<dyn EngineHandoff>,
    id: String,
}

impl RoutedRenderer {
    pub fn new(decider: Box<dyn RenderDecider>, handoff: Box<dyn EngineHandoff>) -> Self {
        let id = format!(
            "routed-renderer-v1;decision={};handoff={}",
            decider.id(),
            handoff.id()
        );
        Self {
            decider,
            handoff,
            id,
        }
    }

    pub fn configured(config: &EnginesConfig) -> Self {
        Self::new(
            Box::new(ConfiguredDecider::new(config)),
            Box::new(ConfiguredHandoff::new(config)),
        )
    }
}

impl Renderer for RoutedRenderer {
    fn id(&self) -> &str {
        &self.id
    }

    fn render(
        &mut self,
        block: &SemanticBlock,
        context: RenderContext,
    ) -> Result<RenderArtifact, RenderError> {
        let decision = self.decider.decide(DecisionRequest::new(block, context))?;
        let response = self
            .handoff
            .execute(EngineRequest::new(decision, block, context))?;
        Ok(response.artifact)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ConfiguredDecider, DecisionRequest, EngineHandoff, EngineRequest, EngineResponse,
        RenderDecider, RenderDecision, RenderRoute, RoutedRenderer,
    };
    use crate::config::{EnginesConfig, MathEngine, MermaidEngine};
    use crate::model::{BlockKind, SemanticBlock};
    use crate::render::{RenderArtifact, RenderContext, RenderError, Renderer};

    fn block(kind: BlockKind) -> SemanticBlock {
        SemanticBlock::new(kind, b"exact source\n".to_vec(), b"body\n".to_vec())
    }

    #[test]
    fn configured_decision_is_separate_for_each_semantic_kind() {
        let mut config = EnginesConfig::default();
        config.mermaid.backend = MermaidEngine::Source;
        config.math.backend = MathEngine::MathjaxCli;
        let decider = ConfiguredDecider::new(&config);

        let mermaid = decider
            .decide(DecisionRequest::new(
                &block(BlockKind::Mermaid),
                RenderContext::default(),
            ))
            .expect("mermaid decision");
        let math = decider
            .decide(DecisionRequest::new(
                &block(BlockKind::Math),
                RenderContext::default(),
            ))
            .expect("math decision");

        assert_eq!(mermaid.route(), RenderRoute::Source);
        assert_eq!(math.route(), RenderRoute::ConfiguredEngine);
    }

    struct FixedDecider;

    impl RenderDecider for FixedDecider {
        fn id(&self) -> &str {
            "test/fixed-decision-v1"
        }

        fn decide(&self, request: DecisionRequest<'_>) -> Result<RenderDecision, RenderError> {
            assert_eq!(request.block().body(), b"body\n");
            assert_eq!(request.context().columns, 123);
            Ok(RenderDecision::new(RenderRoute::ConfiguredEngine))
        }
    }

    struct RecordingHandoff;

    impl EngineHandoff for RecordingHandoff {
        fn id(&self) -> &str {
            "test/recording-handoff-v1"
        }

        fn execute(&mut self, request: EngineRequest<'_>) -> Result<EngineResponse, RenderError> {
            assert_eq!(request.decision().route(), RenderRoute::ConfiguredEngine);
            assert_eq!(request.block().source(), b"exact source\n");
            assert_eq!(request.block().body(), b"body\n");
            assert_eq!(request.context().columns, 123);
            Ok(EngineResponse::new(
                "test/engine-v1",
                RenderArtifact::new(b"rendered\n".to_vec()),
            ))
        }
    }

    #[test]
    fn routed_renderer_preserves_the_typed_handoff_request() {
        let mut renderer = RoutedRenderer::new(
            Box::new(FixedDecider),
            Box::new(RecordingHandoff),
        );
        let artifact = renderer
            .render(
                &block(BlockKind::Math),
                RenderContext {
                    columns: 123,
                    color: false,
                    theme_fingerprint: 7,
                },
            )
            .expect("routed render");

        assert_eq!(artifact.bytes, b"rendered\n");
        assert!(renderer.id().contains("test/fixed-decision-v1"));
        assert!(renderer.id().contains("test/recording-handoff-v1"));
    }
}
