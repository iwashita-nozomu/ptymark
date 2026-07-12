use crate::detector::SemanticDetector;
use crate::model::StreamItem;
use crate::render::{RenderContext, RenderError, RenderService};
use crate::terminal::{OutputSegment, TerminalOutputGate};
use std::error::Error;
use std::fmt;
use std::io::{self, Write};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PipelineReport {
    pub input_bytes: usize,
    pub passthrough_bytes: usize,
    pub raw_terminal_bytes: usize,
    pub semantic_blocks: usize,
    pub rendered_blocks: usize,
    pub fallback_blocks: usize,
    pub cache_hits: usize,
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
        Self {
            gate: TerminalOutputGate::default(),
            detector,
            renderer,
            context,
            strict,
            report: PipelineReport::default(),
        }
    }

    pub fn feed(
        &mut self,
        input: &[u8],
        display: &mut dyn Write,
    ) -> Result<(), PipelineError> {
        self.report.input_bytes = self.report.input_bytes.saturating_add(input.len());

        for segment in self.gate.feed(input) {
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

    pub fn finish(&mut self, display: &mut dyn Write) -> Result<(), PipelineError> {
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

    fn emit(
        &mut self,
        items: Vec<StreamItem>,
        display: &mut dyn Write,
    ) -> Result<(), PipelineError> {
        for item in items {
            match item {
                StreamItem::Passthrough(bytes) => {
                    display.write_all(&bytes)?;
                    self.report.passthrough_bytes =
                        self.report.passthrough_bytes.saturating_add(bytes.len());
                }
                StreamItem::Semantic(block) => {
                    self.report.semantic_blocks = self.report.semantic_blocks.saturating_add(1);
                    match self.renderer.render(&block, self.context) {
                        Ok(output) => {
                            display.write_all(&output.bytes)?;
                            self.report.rendered_blocks =
                                self.report.rendered_blocks.saturating_add(1);
                            if output.cache_hit {
                                self.report.cache_hits =
                                    self.report.cache_hits.saturating_add(1);
                            }
                        }
                        Err(error) if self.strict => {
                            return Err(PipelineError::Render(error));
                        }
                        Err(_) => {
                            display.write_all(block.source())?;
                            self.report.fallback_blocks =
                                self.report.fallback_blocks.saturating_add(1);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DisplayPipeline;
    use crate::cache::NoopCache;
    use crate::config::DetectionConfig;
    use crate::detector::FencedDetector;
    use crate::render::{PreviewRenderer, RenderContext, RenderService};

    #[test]
    fn semantic_block_is_replaced_before_display() {
        let detector = Box::new(FencedDetector::new(&DetectionConfig::default()));
        let renderer =
            RenderService::new(Box::new(PreviewRenderer), Box::new(NoopCache::default()));
        let mut pipeline =
            DisplayPipeline::new(detector, renderer, RenderContext::default(), false);
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
    fn alternate_screen_is_lossless() {
        let source = b"\x1b[?1049h$$\nE = mc^2\n$$\n\x1b[?1049l";
        let detector = Box::new(FencedDetector::new(&DetectionConfig::default()));
        let renderer =
            RenderService::new(Box::new(PreviewRenderer), Box::new(NoopCache::default()));
        let mut pipeline =
            DisplayPipeline::new(detector, renderer, RenderContext::default(), false);
        let mut output = Vec::new();

        for byte in source {
            pipeline.feed(&[*byte], &mut output).expect("feed");
        }
        pipeline.finish(&mut output).expect("finish");
        assert_eq!(output, source);
    }
}
