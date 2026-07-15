use crate::detector::SemanticDetector;
use crate::diagnostics::{DiagnosticComponent, DiagnosticFinding, DiagnosticSeverity, code};
use crate::model::StreamItem;
use crate::render::{RenderCancellation, RenderContext, RenderError, RenderService};
use crate::terminal::{OutputSegment, TerminalOutputGate};
use std::error::Error;
use std::fmt;
use std::io::{self, Write};

pub const MAX_PENDING_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PipelineReport {
    pub input_bytes: usize,
    pub passthrough_bytes: usize,
    pub raw_terminal_bytes: usize,
    pub semantic_blocks: usize,
    pub rendered_blocks: usize,
    pub fallback_blocks: usize,
    pub cache_hits: usize,
    pub findings: Vec<DiagnosticFinding>,
}

#[derive(Debug)]
pub enum PipelineError {
    Render(RenderError),
    Io(io::Error),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Render(error) => write!(formatter, "rendering failed: {error}"),
            Self::Io(error) => write!(formatter, "display output failed: {error}"),
        }
    }
}

impl Error for PipelineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Render(error) => Some(error),
            Self::Io(error) => Some(error),
        }
    }
}

impl From<io::Error> for PipelineError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub struct DisplayPipeline {
    gate: TerminalOutputGate,
    detector: Box<dyn SemanticDetector>,
    renderer: RenderService,
    cancellation: RenderCancellation,
    context: RenderContext,
    strict: bool,
    report: PipelineReport,
}

impl DisplayPipeline {
    pub fn new(
        detector: Box<dyn SemanticDetector>,
        renderer: RenderService,
        context: RenderContext,
        strict: bool,
    ) -> Self {
        Self::with_cancellation(
            detector,
            renderer,
            RenderCancellation::default(),
            context,
            strict,
        )
    }

    pub fn with_cancellation(
        detector: Box<dyn SemanticDetector>,
        renderer: RenderService,
        cancellation: RenderCancellation,
        context: RenderContext,
        strict: bool,
    ) -> Self {
        Self {
            gate: TerminalOutputGate::default(),
            detector,
            renderer,
            cancellation,
            context,
            strict,
            report: PipelineReport::default(),
        }
    }

    pub fn cancellation_handle(&self) -> RenderCancellation {
        self.cancellation.clone()
    }

    pub fn feed(&mut self, input: &[u8], display: &mut dyn Write) -> Result<(), PipelineError> {
        self.report.input_bytes = self.report.input_bytes.saturating_add(input.len());
        let segments = self.gate.feed(input);
        self.emit_segments(segments, display)
    }

    pub fn finish(&mut self, display: &mut dyn Write) -> Result<(), PipelineError> {
        let segments = self.gate.finish();
        self.emit_segments(segments, display)?;
        let items = self.detector.finish();
        self.emit(items, display)?;
        display.flush()?;
        Ok(())
    }

    pub fn report(&self) -> &PipelineReport {
        &self.report
    }

    pub fn cache_stats(&self) -> crate::cache::CacheStats {
        self.renderer.cache_stats()
    }

    pub fn set_columns(&mut self, columns: u16) {
        self.context.columns = columns.max(1);
    }

    fn emit_segments(
        &mut self,
        segments: Vec<OutputSegment>,
        display: &mut dyn Write,
    ) -> Result<(), PipelineError> {
        for segment in segments {
            match segment {
                OutputSegment::SafeText(bytes) => {
                    let items = self.detector.feed(&bytes);
                    self.emit(items, display)?;
                }
                OutputSegment::RawTerminalBytes(bytes) => {
                    let pending = self.detector.finish();
                    self.emit(pending, display)?;
                    display.write_all(&bytes)?;
                    self.report.raw_terminal_bytes =
                        self.report.raw_terminal_bytes.saturating_add(bytes.len());
                }
            }
        }
        Ok(())
    }

    fn emit(
        &mut self,
        items: Vec<StreamItem>,
        display: &mut dyn Write,
    ) -> Result<(), PipelineError> {
        let mut pending_after = vec![0_usize; items.len()];
        let mut pending = 0_usize;
        for (index, item) in items.iter().enumerate().rev() {
            pending_after[index] = pending;
            pending = pending.saturating_add(stream_item_bytes(item));
        }

        for (index, item) in items.into_iter().enumerate() {
            match item {
                StreamItem::Passthrough(bytes) => {
                    display.write_all(&bytes)?;
                    self.report.passthrough_bytes =
                        self.report.passthrough_bytes.saturating_add(bytes.len());
                }
                StreamItem::Semantic(block) => {
                    self.report.semantic_blocks = self.report.semantic_blocks.saturating_add(1);
                    if self.cancellation.is_cancelled()
                        || pending_after[index] > MAX_PENDING_OUTPUT_BYTES
                    {
                        let error = RenderError::coded(
                            code::RENDER_OUTPUT_LIMIT,
                            format!(
                                "pending terminal output exceeded {} bytes while a semantic block was unresolved",
                                MAX_PENDING_OUTPUT_BYTES
                            ),
                        );
                        self.report
                            .findings
                            .push(error.diagnostic_finding(self.strict));
                        if self.strict {
                            self.cancellation.reset();
                            return Err(PipelineError::Render(error));
                        }
                        self.report.findings.push(
                            DiagnosticFinding::new(
                                code::PRESENTATION_FALLBACK,
                                DiagnosticSeverity::Warning,
                                DiagnosticComponent::Presentation,
                                "exact source was restored after pending output exceeded its bound",
                            )
                            .with_remedy(
                                "reduce the producing command's burst size or use source/safe mode",
                            ),
                        );
                        display.write_all(block.source())?;
                        self.report.fallback_blocks = self.report.fallback_blocks.saturating_add(1);
                        self.cancellation.reset();
                        continue;
                    }
                    match self.renderer.render(&block, self.context) {
                        Ok(output) => {
                            display.write_all(&output.bytes)?;
                            self.report.rendered_blocks =
                                self.report.rendered_blocks.saturating_add(1);
                            if output.cache_hit {
                                self.report.cache_hits = self.report.cache_hits.saturating_add(1);
                            }
                            self.cancellation.reset();
                        }
                        Err(error) if self.strict => {
                            self.report.findings.push(error.diagnostic_finding(true));
                            self.cancellation.reset();
                            return Err(PipelineError::Render(error));
                        }
                        Err(error) => {
                            self.report.findings.push(error.diagnostic_finding(false));
                            self.report.findings.push(
                                DiagnosticFinding::new(
                                    code::PRESENTATION_FALLBACK,
                                    DiagnosticSeverity::Warning,
                                    DiagnosticComponent::Presentation,
                                    "exact source was restored after a rendering failure",
                                )
                                .with_remedy(
                                    "use `ptymark doctor` to inspect the selected renderer",
                                ),
                            );
                            display.write_all(block.source())?;
                            self.report.fallback_blocks =
                                self.report.fallback_blocks.saturating_add(1);
                            self.cancellation.reset();
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn stream_item_bytes(item: &StreamItem) -> usize {
    match item {
        StreamItem::Passthrough(bytes) => bytes.len(),
        StreamItem::Semantic(block) => block.source().len(),
    }
}

#[cfg(test)]
mod tests {
    use super::{DisplayPipeline, MAX_PENDING_OUTPUT_BYTES};
    use crate::cache::NoopCache;
    use crate::config::DetectionConfig;
    use crate::detector::FencedDetector;
    use crate::diagnostics::code;
    use crate::model::SemanticBlock;
    use crate::render::{
        PreviewRenderer, RenderArtifact, RenderContext, RenderError, RenderService, Renderer,
    };

    fn preview_pipeline() -> DisplayPipeline {
        let detector = Box::new(FencedDetector::new(&DetectionConfig::default()));
        let renderer =
            RenderService::new(Box::new(PreviewRenderer), Box::new(NoopCache::default()));
        DisplayPipeline::new(detector, renderer, RenderContext::default(), false)
    }

    #[test]
    fn semantic_block_is_replaced_before_display() {
        let mut pipeline = preview_pipeline();
        let mut output = Vec::new();

        pipeline
            .feed(b"before\n$$\nE = mc^2\n$$\nafter\n", &mut output)
            .expect("feed");
        pipeline.finish(&mut output).expect("finish");

        let text = String::from_utf8(output).expect("UTF-8");
        assert!(text.starts_with("before\n"));
        assert!(text.contains("ptymark math"));
        assert!(text.ends_with("after\n"));
        assert!(!text.contains("$$"));
    }

    #[test]
    fn crlf_semantic_block_from_a_pty_is_rendered() {
        let mut pipeline = preview_pipeline();
        let source = b"before\r\n$$\r\nE = mc^2\r\n$$\r\nafter\r\n";
        let mut output = Vec::new();

        for byte in source {
            pipeline.feed(&[*byte], &mut output).expect("feed");
        }
        pipeline.finish(&mut output).expect("finish");

        let text = String::from_utf8(output).expect("UTF-8");
        assert!(text.starts_with("before\r\n"));
        assert!(text.contains("ptymark math"));
        assert!(text.ends_with("after\r\n"));
        assert!(!text.contains("$$"));
    }

    #[test]
    fn alternate_screen_is_lossless() {
        let source = b"\x1b[?1049h$$\nE = mc^2\n$$\n\x1b[?1049l";
        let mut pipeline = preview_pipeline();
        let mut output = Vec::new();

        for byte in source {
            pipeline.feed(&[*byte], &mut output).expect("feed");
        }
        pipeline.finish(&mut output).expect("finish");
        assert_eq!(output, source);
    }

    struct PanicRenderer;

    impl Renderer for PanicRenderer {
        fn id(&self) -> &str {
            "panic-renderer"
        }

        fn render(
            &mut self,
            _block: &SemanticBlock,
            _context: RenderContext,
        ) -> Result<RenderArtifact, RenderError> {
            panic!("renderer must not start when pending output is already over the bound")
        }
    }

    struct TimeoutRenderer;

    impl Renderer for TimeoutRenderer {
        fn id(&self) -> &str {
            "timeout-renderer"
        }

        fn render(
            &mut self,
            _block: &SemanticBlock,
            _context: RenderContext,
        ) -> Result<RenderArtifact, RenderError> {
            Err(RenderError::coded(
                code::RENDER_TIMEOUT,
                "PRIVATE SEMANTIC SOURCE token-123",
            ))
        }
    }

    #[test]
    fn excessive_pending_output_restores_source_without_starting_renderer() {
        let detector = Box::new(FencedDetector::new(&DetectionConfig::default()));
        let renderer = RenderService::new(Box::new(PanicRenderer), Box::new(NoopCache::default()));
        let mut pipeline =
            DisplayPipeline::new(detector, renderer, RenderContext::default(), false);
        let mut input = b"$$\nE = mc^2\n$$\n".to_vec();
        input.extend(std::iter::repeat_n(b'x', MAX_PENDING_OUTPUT_BYTES + 1));
        let mut output = Vec::new();
        pipeline
            .feed(&input, &mut output)
            .expect("bounded fallback");
        pipeline.finish(&mut output).expect("finish");
        assert_eq!(output, input);
        assert_eq!(pipeline.report().fallback_blocks, 1);
        assert!(
            pipeline
                .report()
                .findings
                .iter()
                .any(|finding| finding.code == code::RENDER_OUTPUT_LIMIT)
        );
    }

    #[test]
    fn render_failure_finding_never_copies_source_bearing_detail() {
        let detector = Box::new(FencedDetector::new(&DetectionConfig::default()));
        let renderer =
            RenderService::new(Box::new(TimeoutRenderer), Box::new(NoopCache::default()));
        let mut pipeline =
            DisplayPipeline::new(detector, renderer, RenderContext::default(), false);
        let source = b"$$\nE = mc^2\n$$\n";
        let mut output = Vec::new();
        pipeline.feed(source, &mut output).expect("fallback");
        pipeline.finish(&mut output).expect("finish");
        assert_eq!(output, source);
        let report = pipeline.report();
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == code::RENDER_TIMEOUT)
        );
        let debug = format!("{:?}", report.findings);
        assert!(!debug.contains("PRIVATE SEMANTIC SOURCE"));
        assert!(!debug.contains("token-123"));
    }
}
