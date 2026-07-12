use crate::detector::{DetectError, SemanticDetector};
use crate::model::{DisplayMode, StreamItem};
use crate::renderer::{BlockRenderer, RenderContext, RenderError};
use crate::terminal::{DisplayOutputGate, OutputSegment};
use std::error::Error;
use std::fmt;
use std::io::{self, Write};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PreDisplayReport {
    pub input_bytes: usize,
    pub passthrough_bytes: usize,
    pub semantic_blocks: usize,
    pub rendered_blocks: usize,
    pub fallback_blocks: usize,
    pub bypass_bytes: usize,
    pub diagnostics: Vec<String>,
}

#[derive(Debug)]
pub enum PreDisplayError {
    Detect(DetectError),
    Render(RenderError),
    Io(io::Error),
}

impl fmt::Display for PreDisplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Detect(error) => write!(formatter, "semantic detection failed: {error}"),
            Self::Render(error) => write!(formatter, "semantic rendering failed: {error}"),
            Self::Io(error) => write!(formatter, "display output failed: {error}"),
        }
    }
}

impl Error for PreDisplayError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Detect(error) => Some(error),
            Self::Render(error) => Some(error),
            Self::Io(error) => Some(error),
        }
    }
}

impl From<DetectError> for PreDisplayError {
    fn from(error: DetectError) -> Self {
        Self::Detect(error)
    }
}

impl From<RenderError> for PreDisplayError {
    fn from(error: RenderError) -> Self {
        Self::Render(error)
    }
}

impl From<io::Error> for PreDisplayError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub struct PreDisplayRenderer<D, R> {
    detector: D,
    renderer: R,
    context: RenderContext,
    mode: DisplayMode,
    strict: bool,
    report: PreDisplayReport,
}

impl<D: SemanticDetector, R: BlockRenderer> PreDisplayRenderer<D, R> {
    pub fn new(detector: D, renderer: R) -> Self {
        Self {
            detector,
            renderer,
            context: RenderContext::default(),
            mode: DisplayMode::Transform,
            strict: false,
            report: PreDisplayReport::default(),
        }
    }

    pub fn with_context(mut self, context: RenderContext) -> Self {
        self.context = context;
        self
    }

    pub fn strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    pub const fn mode(&self) -> DisplayMode {
        self.mode
    }

    pub fn report(&self) -> &PreDisplayReport {
        &self.report
    }

    pub fn feed(&mut self, input: &[u8], display: &mut dyn Write) -> Result<(), PreDisplayError> {
        self.report.input_bytes = self.report.input_bytes.saturating_add(input.len());

        if self.mode == DisplayMode::Bypass {
            display.write_all(input)?;
            self.report.bypass_bytes = self.report.bypass_bytes.saturating_add(input.len());
            return Ok(());
        }

        let items = self.detector.feed(input)?;
        self.emit(items, display)
    }

    pub fn set_mode(
        &mut self,
        mode: DisplayMode,
        display: &mut dyn Write,
    ) -> Result<(), PreDisplayError> {
        if self.mode == mode {
            return Ok(());
        }

        if self.mode == DisplayMode::Transform && mode == DisplayMode::Bypass {
            let pending = self.detector.finish()?;
            self.emit(pending, display)?;
        }

        self.mode = mode;
        Ok(())
    }

    pub fn finish(&mut self, display: &mut dyn Write) -> Result<(), PreDisplayError> {
        if self.mode == DisplayMode::Transform {
            let items = self.detector.finish()?;
            self.emit(items, display)?;
        }
        display.flush()?;
        Ok(())
    }

    fn emit(
        &mut self,
        items: Vec<StreamItem>,
        display: &mut dyn Write,
    ) -> Result<(), PreDisplayError> {
        for item in items {
            match item {
                StreamItem::Passthrough(bytes) => {
                    display.write_all(&bytes)?;
                    self.report.passthrough_bytes =
                        self.report.passthrough_bytes.saturating_add(bytes.len());
                }
                StreamItem::Semantic(block) => {
                    self.report.semantic_blocks = self.report.semantic_blocks.saturating_add(1);
                    match self.renderer.render(&block, &self.context) {
                        Ok(rendered) => {
                            display.write_all(&rendered)?;
                            self.report.rendered_blocks =
                                self.report.rendered_blocks.saturating_add(1);
                        }
                        Err(error) if self.strict => return Err(PreDisplayError::Render(error)),
                        Err(error) => {
                            display.write_all(block.source())?;
                            self.report.fallback_blocks =
                                self.report.fallback_blocks.saturating_add(1);
                            self.report.diagnostics.push(error.to_string());
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub struct DisplayInterceptor<G, D, R> {
    gate: G,
    pre_display: PreDisplayRenderer<D, R>,
}

impl<G: DisplayOutputGate, D: SemanticDetector, R: BlockRenderer> DisplayInterceptor<G, D, R> {
    pub fn new(gate: G, pre_display: PreDisplayRenderer<D, R>) -> Self {
        Self { gate, pre_display }
    }

    pub fn feed(&mut self, input: &[u8], display: &mut dyn Write) -> Result<(), PreDisplayError> {
        let segments = self.gate.feed(input);
        self.emit_segments(segments, display)
    }

    pub fn finish(&mut self, display: &mut dyn Write) -> Result<(), PreDisplayError> {
        let segments = self.gate.finish();
        self.emit_segments(segments, display)?;
        self.pre_display.finish(display)
    }

    pub fn report(&self) -> &PreDisplayReport {
        self.pre_display.report()
    }

    fn emit_segments(
        &mut self,
        segments: Vec<OutputSegment>,
        display: &mut dyn Write,
    ) -> Result<(), PreDisplayError> {
        for segment in segments {
            match segment {
                OutputSegment::SafeText(bytes) => {
                    self.pre_display.set_mode(DisplayMode::Transform, display)?;
                    self.pre_display.feed(&bytes, display)?;
                }
                OutputSegment::RawTerminalBytes(bytes) => {
                    self.pre_display.set_mode(DisplayMode::Bypass, display)?;
                    self.pre_display.feed(&bytes, display)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{DisplayInterceptor, PreDisplayRenderer};
    use crate::detector::FencedDetector;
    use crate::model::{DisplayMode, SemanticBlock};
    use crate::renderer::{BlockRenderer, RenderContext, RenderError};
    use crate::terminal::TerminalOutputGate;

    #[derive(Debug, Default)]
    struct KindRenderer;

    impl BlockRenderer for KindRenderer {
        fn render(
            &mut self,
            block: &SemanticBlock,
            _context: &RenderContext,
        ) -> Result<Vec<u8>, RenderError> {
            Ok(format!("<{}>", block.kind()).into_bytes())
        }
    }

    #[test]
    fn transforms_before_bytes_reach_display_writer() {
        let mut renderer = PreDisplayRenderer::new(FencedDetector::new(1024), KindRenderer);
        let mut display = Vec::new();
        renderer
            .feed(b"before\n$$\nE = mc^2\n$$\nafter\n", &mut display)
            .expect("feed");
        renderer.finish(&mut display).expect("finish");

        assert_eq!(display, b"before\n<math>after\n");
        assert_eq!(renderer.report().rendered_blocks, 1);
    }

    #[test]
    fn bypass_mode_flushes_pending_source_before_direct_output() {
        let mut renderer = PreDisplayRenderer::new(FencedDetector::new(1024), KindRenderer);
        let mut display = Vec::new();
        renderer
            .feed(b"```mermaid\nA -->", &mut display)
            .expect("feed");
        assert!(display.is_empty());

        renderer
            .set_mode(DisplayMode::Bypass, &mut display)
            .expect("mode switch");
        renderer
            .feed(b" B\n```\n", &mut display)
            .expect("bypass feed");
        renderer.finish(&mut display).expect("finish");

        assert_eq!(display, b"```mermaid\nA --> B\n```\n");
        assert_eq!(renderer.report().rendered_blocks, 0);
    }

    #[test]
    fn terminal_control_forces_lossless_display_bypass() {
        let source = b"before\n\x1b[?1049h$$\nE = mc^2\n$$\n\x1b[?1049lafter\n";
        let pre_display = PreDisplayRenderer::new(FencedDetector::new(1024), KindRenderer);
        let mut interceptor = DisplayInterceptor::new(TerminalOutputGate::default(), pre_display);
        let mut display = Vec::new();
        for chunk in source.chunks(1) {
            interceptor.feed(chunk, &mut display).expect("feed");
        }
        interceptor.finish(&mut display).expect("finish");
        assert_eq!(display, source);
        assert_eq!(interceptor.report().rendered_blocks, 0);
    }
}
