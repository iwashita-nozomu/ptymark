use crate::model::{BlockKind, SemanticBlock, SemanticFormat};
use crate::openmath::{OPENMATH_TO_TEX_ID, to_tex};
use crate::render::{RenderArtifact, RenderContext, RenderError, Renderer};

/// Converts structured source formats into the body protocol expected by an
/// existing semantic renderer while preserving the original source bytes.
pub struct OpenMathAdapterRenderer {
    inner: Box<dyn Renderer>,
    enabled: bool,
    id: String,
}

impl OpenMathAdapterRenderer {
    pub fn new(inner: Box<dyn Renderer>, enabled: bool) -> Self {
        let id = format!(
            "openmath-adapter-v1;enabled={enabled};converter={OPENMATH_TO_TEX_ID};inner={}",
            inner.id()
        );
        Self { inner, enabled, id }
    }
}

impl Renderer for OpenMathAdapterRenderer {
    fn id(&self) -> &str {
        &self.id
    }

    fn render(
        &mut self,
        block: &SemanticBlock,
        context: RenderContext,
    ) -> Result<RenderArtifact, RenderError> {
        if !self.enabled || block.format() != SemanticFormat::OpenMath {
            return self.inner.render(block, context);
        }

        let tex = to_tex(block.body())
            .map_err(|error| RenderError::new(format!("OpenMath input is invalid: {error}")))?;
        let adapted = SemanticBlock::with_format(
            BlockKind::Math,
            SemanticFormat::OpenMath,
            block.source().to_vec(),
            tex.into_bytes(),
        );
        self.inner.render(&adapted, context)
    }
}

#[cfg(test)]
mod tests {
    use super::OpenMathAdapterRenderer;
    use crate::model::{BlockKind, SemanticBlock, SemanticFormat};
    use crate::render::{PreviewRenderer, RenderContext, Renderer, SourceRenderer};

    #[test]
    fn converts_openmath_before_the_existing_math_renderer() {
        let block = SemanticBlock::with_format(
            BlockKind::Math,
            SemanticFormat::OpenMath,
            b"```openmath\nsource\n```\n".to_vec(),
            b"<OMOBJ xmlns=\"http://www.openmath.org/OpenMath\"><OMA><OMS cd=\"arith1\" name=\"plus\"/><OMV name=\"x\"/><OMI>1</OMI></OMA></OMOBJ>\n".to_vec(),
        );
        let mut renderer = OpenMathAdapterRenderer::new(Box::new(PreviewRenderer), true);
        let artifact = renderer
            .render(&block, RenderContext::default())
            .expect("OpenMath preview");
        let output = String::from_utf8(artifact.bytes).expect("UTF-8 preview");
        assert!(output.contains("x + 1"));
        assert!(renderer.id().contains("builtin/openmath-to-tex-v1"));
    }

    #[test]
    fn disabled_adapter_preserves_invalid_openmath_source() {
        let source = b"```openmath\n<not-openmath/>\n```\n";
        let block = SemanticBlock::with_format(
            BlockKind::Math,
            SemanticFormat::OpenMath,
            source.to_vec(),
            b"<not-openmath/>\n".to_vec(),
        );
        let mut renderer = OpenMathAdapterRenderer::new(Box::new(SourceRenderer), false);
        let artifact = renderer
            .render(&block, RenderContext::default())
            .expect("source output");
        assert_eq!(artifact.bytes, source);
    }
}
