use ptymark::{
    BlockRenderer, DisplayInterceptor, DisplayMode, FencedDetector, PreDisplayRenderer,
    RenderContext, RenderError, SemanticBlock, SourceRenderer, TerminalOutputGate,
};

#[derive(Debug, Default)]
struct MarkerRenderer;

impl BlockRenderer for MarkerRenderer {
    fn render(
        &mut self,
        block: &SemanticBlock,
        _context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError> {
        Ok(format!("<rendered:{}>", block.kind()).into_bytes())
    }
}

#[derive(Debug, Default)]
struct FailingRenderer;

impl BlockRenderer for FailingRenderer {
    fn render(
        &mut self,
        _block: &SemanticBlock,
        _context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError> {
        Err(RenderError::new("intentional test failure"))
    }
}

fn run_in_chunks<R: BlockRenderer>(
    renderer: R,
    source: &[u8],
    chunk_size: usize,
) -> (Vec<u8>, ptymark::PreDisplayReport) {
    let pre_display = PreDisplayRenderer::new(FencedDetector::new(1024), renderer);
    let mut pipeline = DisplayInterceptor::new(TerminalOutputGate::default(), pre_display);
    let mut display = Vec::new();
    for chunk in source.chunks(chunk_size) {
        pipeline.feed(chunk, &mut display).expect("feed");
    }
    pipeline.finish(&mut display).expect("finish");
    (display, pipeline.report().clone())
}

#[test]
fn ordinary_output_reaches_display_byte_for_byte() {
    let source = b"plain\x1b[31m red\x1b[0m\rprogress";
    let (display, report) = run_in_chunks(MarkerRenderer, source, 3);
    assert_eq!(display, source);
    assert_eq!(report.semantic_blocks, 0);
    assert_eq!(report.input_bytes, source.len());
}

#[test]
fn complete_semantic_block_is_replaced_before_display() {
    let source = b"before\n$$\nE = mc^2\n$$\nafter\n";
    let (display, report) = run_in_chunks(MarkerRenderer, source, 2);
    assert_eq!(display, b"before\n<rendered:math>after\n");
    assert!(!display.windows(2).any(|window| window == b"$$"));
    assert_eq!(report.semantic_blocks, 1);
    assert_eq!(report.rendered_blocks, 1);
}

#[test]
fn chunk_boundaries_do_not_change_display_result() {
    let source = b"a\n```mermaid\nA --> B\n```\nz\n";
    let (one_byte, _) = run_in_chunks(MarkerRenderer, source, 1);
    let (whole, _) = run_in_chunks(MarkerRenderer, source, source.len());
    assert_eq!(one_byte, whole);
    assert_eq!(whole, b"a\n<rendered:mermaid>z\n");
}

#[test]
fn renderer_failure_restores_original_source_by_default() {
    let source = b"before\n```mermaid\nA --> B\n```\nafter\n";
    let (display, report) = run_in_chunks(FailingRenderer, source, 5);
    assert_eq!(display, source);
    assert_eq!(report.fallback_blocks, 1);
    assert_eq!(report.diagnostics, vec!["intentional test failure"]);
}

#[test]
fn strict_mode_surfaces_renderer_error() {
    let mut pipeline =
        PreDisplayRenderer::new(FencedDetector::new(1024), FailingRenderer).strict(true);
    let mut display = Vec::new();
    let error = pipeline
        .feed(b"$$\nE = mc^2\n$$\n", &mut display)
        .expect_err("strict mode must fail");
    assert!(error.to_string().contains("intentional test failure"));
    assert!(display.is_empty());
}

#[test]
fn incomplete_block_is_never_hidden() {
    let source = b"before\n```mermaid\nA --> B\n";
    let (display, report) = run_in_chunks(MarkerRenderer, source, 4);
    assert_eq!(display, source);
    assert_eq!(report.rendered_blocks, 0);
}

#[test]
fn buffer_limit_degrades_to_lossless_passthrough() {
    let source = b"$$\n123456789\n$$\n";
    let mut pipeline = PreDisplayRenderer::new(FencedDetector::new(8), MarkerRenderer);
    let mut display = Vec::new();
    pipeline.feed(source, &mut display).expect("feed");
    pipeline.finish(&mut display).expect("finish");
    assert_eq!(display, source);
    assert_eq!(pipeline.report().rendered_blocks, 0);
}

#[test]
fn bypass_mode_flushes_pending_source_before_direct_display() {
    let mut pipeline = PreDisplayRenderer::new(FencedDetector::new(1024), MarkerRenderer);
    let mut display = Vec::new();
    pipeline
        .feed(b"```mermaid\nA -->", &mut display)
        .expect("feed");
    pipeline
        .set_mode(DisplayMode::Bypass, &mut display)
        .expect("switch to bypass");
    pipeline
        .feed(b" B\n```\n", &mut display)
        .expect("bypass feed");
    pipeline.finish(&mut display).expect("finish");
    assert_eq!(display, b"```mermaid\nA --> B\n```\n");
    assert_eq!(pipeline.report().bypass_bytes, b" B\n```\n".len());
}

#[test]
fn source_renderer_proves_lossless_replacement_path() {
    let source = b"x\n$$\nE = mc^2\n$$\ny\n";
    let (display, report) = run_in_chunks(SourceRenderer, source, 7);
    assert_eq!(display, source);
    assert_eq!(report.rendered_blocks, 1);
}
