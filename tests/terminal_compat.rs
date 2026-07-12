use ptymark::{
    BlockRenderer, DisplayInterceptor, FencedDetector, PreDisplayRenderer, RenderContext,
    RenderError, SemanticBlock, TerminalOutputGate,
};

#[derive(Default)]
struct Marker;

impl BlockRenderer for Marker {
    fn render(
        &mut self,
        block: &SemanticBlock,
        _context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError> {
        Ok(format!("<{}>", block.kind()).into_bytes())
    }
}

fn intercept(source: &[u8], chunk_size: usize) -> Vec<u8> {
    let pre_display = PreDisplayRenderer::new(FencedDetector::new(4096), Marker);
    let mut interceptor = DisplayInterceptor::new(TerminalOutputGate::default(), pre_display);
    let mut output = Vec::new();
    for chunk in source.chunks(chunk_size) {
        interceptor.feed(chunk, &mut output).expect("feed");
    }
    interceptor.finish(&mut output).expect("finish");
    output
}

#[test]
fn ansi_sgr_is_byte_exact() {
    let source = b"prefix \x1b[1;31mred\x1b[0m suffix\n";
    assert_eq!(intercept(source, 1), source);
}

#[test]
fn osc_hyperlink_and_shell_markers_are_byte_exact() {
    let source = b"\x1b]8;;https://example.com\x07link\x1b]8;;\x07\n\x1b]133;A\x07prompt\n";
    assert_eq!(intercept(source, 2), source);
}

#[test]
fn dcs_and_unknown_escape_sequences_are_byte_exact() {
    let source = b"\x1bP1;2;3+qpayload\x1b\\\x1b[?2026htext\x1b[?2026l\n";
    assert_eq!(intercept(source, 3), source);
}

#[test]
fn carriage_return_progress_output_is_byte_exact() {
    let source = b"progress 10%\rprogress 20%\rprogress 100%\n";
    assert_eq!(intercept(source, 1), source);
}

#[test]
fn alternate_screen_disables_semantic_detection() {
    let source = b"before\n\x1b[?1049h$$\nE = mc^2\n$$\n\x1b[?1049lafter\n";
    assert_eq!(intercept(source, 1), source);
}

#[test]
fn safe_block_outside_control_regions_is_still_rendered() {
    let source = b"before\n$$\nE = mc^2\n$$\nafter\n";
    assert_eq!(intercept(source, 1), b"before\n<math>after\n");
}
