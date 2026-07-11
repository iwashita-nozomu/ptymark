use ptymark::{
    ArtifactFormat, BlockKind, CachePolicy, EngineDescriptor, EngineRegistry, ExecutionModel,
    LayoutSensitivity, MemoryArtifactCache, PolicyEngineSelector, RenderArtifact, RenderContext,
    RenderCoordinator, RenderEngine, RenderError, RenderRequest, SemanticBlock,
};
use std::time::Instant;

struct BenchEngine {
    descriptor: EngineDescriptor,
}

impl Default for BenchEngine {
    fn default() -> Self {
        Self {
            descriptor: EngineDescriptor::new(
                "bench/in-process",
                "1",
                vec![BlockKind::Math],
                vec![ArtifactFormat::Svg],
                LayoutSensitivity::Columns,
                ExecutionModel::InProcess,
            ),
        }
    }
}

impl RenderEngine for BenchEngine {
    fn descriptor(&self) -> &EngineDescriptor {
        &self.descriptor
    }

    fn render(&mut self, request: &RenderRequest<'_>) -> Result<RenderArtifact, RenderError> {
        Ok(RenderArtifact::new(
            ArtifactFormat::Svg,
            b"<svg viewBox=\"0 0 1 1\"/>".to_vec(),
            self.descriptor.identity.clone(),
            request.block.kind(),
            self.descriptor.layout_sensitivity,
        ))
    }
}

fn percentile(sorted: &[u128], fraction: f64) -> u128 {
    let index = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[index]
}

fn main() {
    let mut registry = EngineRegistry::new();
    registry.register(BenchEngine::default()).expect("register");
    let selector =
        PolicyEngineSelector::new().with_candidates(BlockKind::Math, ["bench/in-process"]);
    let mut coordinator = RenderCoordinator::new(
        registry,
        selector,
        MemoryArtifactCache::new(CachePolicy::new(256, 4 * 1024 * 1024)),
    );
    let block = SemanticBlock::new(
        BlockKind::Math,
        b"$$\nE = mc^2\n$$\n".to_vec(),
        b"E = mc^2\n".to_vec(),
    );
    let context = RenderContext {
        terminal_width: Some(100),
        ..RenderContext::default()
    };

    coordinator
        .render(
            &block,
            &context,
            &[ArtifactFormat::Svg],
            "bench/svg",
            1,
        )
        .expect("prime cache");

    let iterations = 20_000;
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        coordinator
            .render(
                &block,
                &context,
                &[ArtifactFormat::Svg],
                "bench/svg",
                1,
            )
            .expect("cache hit");
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    let total: u128 = samples.iter().sum();
    let mean = total / samples.len() as u128;

    println!(
        "{{\"schema\":\"ptymark.core-benchmark.v1\",\"iterations\":{iterations},\"cache_hit_ns\":{{\"mean\":{mean},\"p50\":{},\"p95\":{},\"max\":{}}}}}",
        percentile(&samples, 0.50),
        percentile(&samples, 0.95),
        samples.last().copied().unwrap_or_default(),
    );
}
