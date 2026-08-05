use crate::detector::SemanticDetector;
use crate::diagnostics::{DiagnosticComponent, DiagnosticFinding, DiagnosticSeverity, code};
use crate::model::StreamItem;
use crate::render::{RenderCancellation, RenderContext, RenderError, RenderService};
use crate::terminal::{OutputSegment, TerminalOutputGate};
use std::error::Error;
use std::fmt;
use std::io::{self, Write};

pub const MAX_PENDING_OUTPUT_BYTES: usize = crate::limits::MAX_PENDING_TERMINAL_BYTES;

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
    terminal_line_endings: bool,
    rendering_enabled: bool,
    requested_rendering_enabled: bool,
    safe_line_start: bool,
    passthrough_until_newline: bool,
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
            terminal_line_endings: false,
            rendering_enabled: true,
            requested_rendering_enabled: true,
            safe_line_start: true,
            passthrough_until_newline: false,
            report: PipelineReport::default(),
        }
    }

    pub fn cancellation_handle(&self) -> RenderCancellation {
        self.cancellation.clone()
    }

    pub fn feed(&mut self, input: &[u8], display: &mut dyn Write) -> Result<(), PipelineError> {
        self.apply_rendering_state(display)?;
        self.report.input_bytes = self.report.input_bytes.saturating_add(input.len());
        let segments = self.gate.feed(input);
        self.emit_segments(segments, display)
    }

    pub fn finish(&mut self, display: &mut dyn Write) -> Result<(), PipelineError> {
        self.apply_rendering_state(display)?;
        let segments = self.gate.finish();
        self.emit_segments(segments, display)?;
        let items = self.detector.finish();
        if self.rendering_enabled {
            self.emit(items, display)?;
        } else {
            self.emit_exact_source(items, display)?;
        }
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

    pub fn set_terminal_line_endings(&mut self, enabled: bool) {
        self.terminal_line_endings = enabled;
    }

    /// Request semantic rendering to be enabled or disabled at the next safe
    /// display boundary. Terminal-control classification remains active in
    /// both states.
    pub(crate) fn set_rendering_enabled(&mut self, enabled: bool) {
        self.requested_rendering_enabled = enabled;
    }

    fn apply_rendering_state(&mut self, display: &mut dyn Write) -> Result<(), PipelineError> {
        if self.rendering_enabled == self.requested_rendering_enabled {
            return Ok(());
        }
        if !self.requested_rendering_enabled {
            let pending = self.detector.finish();
            self.emit_exact_source(pending, display)?;
        } else {
            self.passthrough_until_newline = !self.safe_line_start;
        }
        self.rendering_enabled = self.requested_rendering_enabled;
        Ok(())
    }

    fn emit_segments(
        &mut self,
        segments: Vec<OutputSegment>,
        display: &mut dyn Write,
    ) -> Result<(), PipelineError> {
        for segment in segments {
            match segment {
                OutputSegment::SafeText(bytes) if self.rendering_enabled => {
                    self.emit_renderable_text(&bytes, display)?;
                    self.update_safe_line_start(&bytes);
                }
                OutputSegment::SafeText(bytes) => {
                    self.write_passthrough(&bytes, display)?;
                    self.update_safe_line_start(&bytes);
                }
                OutputSegment::RawTerminalBytes(bytes) => {
                    let pending = self.detector.finish();
                    if self.rendering_enabled {
                        self.emit(pending, display)?;
                    } else {
                        self.emit_exact_source(pending, display)?;
                    }
                    display.write_all(&bytes)?;
                    self.report.raw_terminal_bytes =
                        self.report.raw_terminal_bytes.saturating_add(bytes.len());
                    self.update_safe_line_start(&bytes);
                }
            }
        }
        Ok(())
    }

    fn emit_renderable_text(
        &mut self,
        bytes: &[u8],
        display: &mut dyn Write,
    ) -> Result<(), PipelineError> {
        let renderable = if self.passthrough_until_newline {
            match bytes.iter().position(|byte| *byte == b'\n') {
                Some(index) => {
                    let split = index + 1;
                    self.write_passthrough(&bytes[..split], display)?;
                    self.passthrough_until_newline = false;
                    &bytes[split..]
                }
                None => {
                    self.write_passthrough(bytes, display)?;
                    return Ok(());
                }
            }
        } else {
            bytes
        };
        let items = self.detector.feed(renderable);
        self.emit(items, display)
    }

    fn update_safe_line_start(&mut self, bytes: &[u8]) {
        if let Some(byte) = bytes.last() {
            self.safe_line_start = *byte == b'\n';
            if self.safe_line_start {
                self.passthrough_until_newline = false;
            }
        }
    }

    fn emit_exact_source(
        &mut self,
        items: Vec<StreamItem>,
        display: &mut dyn Write,
    ) -> Result<(), PipelineError> {
        for item in items {
            match item {
                StreamItem::Passthrough(bytes) => self.write_passthrough(&bytes, display)?,
                StreamItem::Semantic(block) => {
                    self.report.semantic_blocks = self.report.semantic_blocks.saturating_add(1);
                    self.write_passthrough(block.source(), display)?;
                }
            }
        }
        Ok(())
    }

    fn write_passthrough(
        &mut self,
        bytes: &[u8],
        display: &mut dyn Write,
    ) -> Result<(), PipelineError> {
        display.write_all(bytes)?;
        self.report.passthrough_bytes = self.report.passthrough_bytes.saturating_add(bytes.len());
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
                    self.write_passthrough(&bytes, display)?;
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
                            write_rendered_output(
                                &output.bytes,
                                self.terminal_line_endings,
                                display,
                            )?;
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

fn write_rendered_output(
    bytes: &[u8],
    terminal_line_endings: bool,
    display: &mut dyn Write,
) -> io::Result<()> {
    if !terminal_line_endings {
        return display.write_all(bytes);
    }

    let mut start = 0_usize;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'\n' || (index > 0 && bytes[index - 1] == b'\r') {
            continue;
        }
        display.write_all(&bytes[start..index])?;
        display.write_all(b"\r\n")?;
        start = index + 1;
    }
    display.write_all(&bytes[start..])
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

        assert!(!output.contains(&b'\r'));
        let text = String::from_utf8(output).expect("UTF-8");
        assert!(text.starts_with("before\n"));
        assert!(text.contains("ptymark math"));
        assert!(text.ends_with("after\n"));
        assert!(!text.contains("$$"));
    }

    #[test]
    fn terminal_line_endings_apply_only_to_rendered_output() {
        let mut pipeline = preview_pipeline();
        pipeline.set_terminal_line_endings(true);
        let mut output = Vec::new();

        pipeline
            .feed(b"before\n$$\nE = mc^2\n$$\nafter\n", &mut output)
            .expect("feed");
        pipeline.finish(&mut output).expect("finish");

        let text = String::from_utf8(output).expect("UTF-8");
        assert!(text.starts_with("before\n"));
        assert!(text.ends_with("after\n"));
        let rendered = text
            .strip_prefix("before\n")
            .and_then(|value| value.strip_suffix("after\n"))
            .expect("rendered middle");
        assert!(rendered.contains("ptymark math"));
        assert!(rendered.contains("\r\n"));
        assert!(
            !rendered
                .as_bytes()
                .iter()
                .enumerate()
                .any(|(index, byte)| *byte == b'\n'
                    && (index == 0 || rendered.as_bytes()[index - 1] != b'\r'))
        );
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

    #[test]
    fn disabling_rendering_restores_a_partial_block_as_exact_source() {
        let mut pipeline = preview_pipeline();
        let mut output = Vec::new();
        let source = b"before\n$$\nE = mc^2\n$$\nafter\n";

        pipeline
            .feed(b"before\n$$\nE =", &mut output)
            .expect("partial feed");
        pipeline.set_rendering_enabled(false);
        pipeline
            .feed(b" mc^2\n$$\nafter\n", &mut output)
            .expect("disabled feed");
        pipeline.finish(&mut output).expect("finish");

        assert_eq!(output, source);
        assert_eq!(pipeline.report().rendered_blocks, 0);
    }

    #[test]
    fn rendering_can_resume_for_future_blocks() {
        let mut pipeline = preview_pipeline();
        let mut output = Vec::new();

        pipeline.set_rendering_enabled(false);
        pipeline
            .feed(b"$$\nA = 1\n$$\n", &mut output)
            .expect("disabled block");
        pipeline.set_rendering_enabled(true);
        pipeline
            .feed(b"$$\nB = 2\n$$\n", &mut output)
            .expect("enabled block");
        pipeline.finish(&mut output).expect("finish");

        let text = String::from_utf8(output).expect("UTF-8");
        assert!(text.starts_with("$$\nA = 1\n$$\n"), "{text}");
        assert!(text.contains("ptymark math"), "{text}");
        assert!(!text.contains("$$\nB = 2\n$$"), "{text}");
        assert_eq!(pipeline.report().rendered_blocks, 1);
    }

    #[test]
    fn a_raw_newline_restores_a_safe_resume_boundary() {
        let mut pipeline = preview_pipeline();
        let mut output = Vec::new();

        pipeline.set_rendering_enabled(false);
        pipeline
            .feed(b"prefix\x1b[31mred\x1b[0m\n", &mut output)
            .expect("disabled raw line");
        pipeline.set_rendering_enabled(true);
        pipeline
            .feed(b"$$\nB = 2\n$$\n", &mut output)
            .expect("enabled block");
        pipeline.finish(&mut output).expect("finish");

        let text = String::from_utf8(output).expect("UTF-8");
        assert!(text.starts_with("prefix\x1b[31mred\x1b[0m\n"), "{text:?}");
        assert!(text.contains("ptymark math"), "{text}");
        assert!(!text.contains("$$\nB = 2\n$$"), "{text}");
    }

    #[test]
    fn reenable_mid_line_does_not_create_a_false_block_boundary() {
        let mut pipeline = preview_pipeline();
        let mut output = Vec::new();
        let source = b"prefix$$\nA = 1\n$$\n";

        pipeline.set_rendering_enabled(false);
        pipeline
            .feed(b"prefix", &mut output)
            .expect("disabled prefix");
        pipeline.set_rendering_enabled(true);
        pipeline
            .feed(b"$$\nA = 1\n$$\n", &mut output)
            .expect("enabled remainder");
        pipeline.finish(&mut output).expect("finish");

        assert_eq!(output, source);
        assert_eq!(pipeline.report().rendered_blocks, 0);
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
