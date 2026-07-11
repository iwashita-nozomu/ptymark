use crate::model::SemanticBlock;
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderContext {
    pub color: bool,
    pub terminal_width: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderError {
    message: String,
}

impl RenderError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for RenderError {}

pub trait BlockRenderer: Send {
    fn render(
        &mut self,
        block: &SemanticBlock,
        context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError>;
}

impl<T: BlockRenderer + ?Sized> BlockRenderer for Box<T> {
    fn render(
        &mut self,
        block: &SemanticBlock,
        context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError> {
        (**self).render(block, context)
    }
}

#[derive(Debug, Default)]
pub struct PreviewRenderer;

impl BlockRenderer for PreviewRenderer {
    fn render(
        &mut self,
        block: &SemanticBlock,
        context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError> {
        let mut output = Vec::new();
        if context.color {
            output.extend_from_slice(b"\x1b[1;36m");
        }
        output.extend_from_slice(format!("┌─ ptymark {} preview ─\n", block.kind()).as_bytes());
        if context.color {
            output.extend_from_slice(b"\x1b[0m");
        }

        let body = String::from_utf8_lossy(block.body());
        if body.is_empty() {
            output.extend_from_slice("│ <empty>\n".as_bytes());
        } else {
            for line in body.split_inclusive('\n') {
                let line = line.strip_suffix('\n').unwrap_or(line);
                let line = line.strip_suffix('\r').unwrap_or(line);
                output.extend_from_slice("│ ".as_bytes());
                output.extend_from_slice(line.as_bytes());
                output.push(b'\n');
            }
        }
        output.extend_from_slice("└─ end preview\n".as_bytes());
        Ok(output)
    }
}

#[derive(Debug, Default)]
pub struct SourceRenderer;

impl BlockRenderer for SourceRenderer {
    fn render(
        &mut self,
        block: &SemanticBlock,
        _context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError> {
        Ok(block.source().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockRenderer, PreviewRenderer, RenderContext, SourceRenderer};
    use crate::model::{BlockKind, SemanticBlock};

    fn block() -> SemanticBlock {
        SemanticBlock::new(
            BlockKind::Mermaid,
            b"```mermaid\nA --> B\n```\n".to_vec(),
            b"A --> B\n".to_vec(),
        )
    }

    #[test]
    fn preview_renderer_emits_display_bytes_without_source_fence() {
        let mut renderer = PreviewRenderer;
        let rendered = renderer
            .render(&block(), &RenderContext::default())
            .expect("render");
        let text = String::from_utf8(rendered).expect("UTF-8 preview");
        assert!(text.contains("mermaid preview"));
        assert!(text.contains("A --> B"));
        assert!(!text.contains("```mermaid"));
    }

    #[test]
    fn source_renderer_is_lossless() {
        let block = block();
        let mut renderer = SourceRenderer;
        let rendered = renderer
            .render(&block, &RenderContext::default())
            .expect("render");
        assert_eq!(rendered, block.source());
    }
}
