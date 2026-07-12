use ptymark::{
    BlockKind, DecisionRequest, EngineHandoff, EngineRequest, EngineResponse, RenderArtifact,
    RenderContext, RenderDecider, RenderDecision, RenderError, RenderRoute, Renderer,
    RoutedRenderer, SemanticBlock,
};
use std::sync::{Arc, Mutex};

type Observation = (RenderRoute, BlockKind, Vec<u8>, u16);
type Observations = Arc<Mutex<Vec<Observation>>>;

struct SizeAwareDecider;

impl RenderDecider for SizeAwareDecider {
    fn id(&self) -> &str {
        "example/size-aware-v1"
    }

    fn decide(&self, request: DecisionRequest<'_>) -> Result<RenderDecision, RenderError> {
        let route = if request.block().body().len() > 16 {
            RenderRoute::Source
        } else {
            RenderRoute::ConfiguredEngine
        };
        Ok(RenderDecision::new(route))
    }
}

struct RecordingHandoff {
    seen: Observations,
}

impl EngineHandoff for RecordingHandoff {
    fn id(&self) -> &str {
        "example/recording-handoff-v1"
    }

    fn execute(&mut self, request: EngineRequest<'_>) -> Result<EngineResponse, RenderError> {
        self.seen.lock().expect("recording lock").push((
            request.decision().route(),
            request.block().kind(),
            request.block().body().to_vec(),
            request.context().columns,
        ));
        Ok(EngineResponse::new(
            "example/engine-v1",
            RenderArtifact::new(b"engine output\n".to_vec()),
        ))
    }
}

#[test]
fn custom_decision_and_handoff_can_be_substituted_without_changing_the_pipeline_api() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut renderer = RoutedRenderer::new(
        Box::new(SizeAwareDecider),
        Box::new(RecordingHandoff {
            seen: Arc::clone(&seen),
        }),
    );
    let block = SemanticBlock::new(
        BlockKind::Mermaid,
        b"```mermaid\nA --> B\n```\n".to_vec(),
        b"A --> B\n".to_vec(),
    );

    let artifact = renderer
        .render(
            &block,
            RenderContext {
                columns: 96,
                color: false,
                theme_fingerprint: 11,
            },
        )
        .expect("routed rendering");

    assert_eq!(artifact.bytes, b"engine output\n");
    assert_eq!(
        seen.lock().expect("recording lock").as_slice(),
        &[(
            RenderRoute::ConfiguredEngine,
            BlockKind::Mermaid,
            b"A --> B\n".to_vec(),
            96,
        )]
    );
}

#[test]
fn decision_can_change_without_exposing_terminal_stream_bytes_to_the_handoff() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut renderer = RoutedRenderer::new(
        Box::new(SizeAwareDecider),
        Box::new(RecordingHandoff {
            seen: Arc::clone(&seen),
        }),
    );
    let block = SemanticBlock::new(
        BlockKind::Math,
        b"$$\n0123456789abcdefghijkl\n$$\n".to_vec(),
        b"0123456789abcdefghijkl\n".to_vec(),
    );

    renderer
        .render(&block, RenderContext::default())
        .expect("routed rendering");

    assert_eq!(
        seen.lock().expect("recording lock")[0].0,
        RenderRoute::Source
    );
}
