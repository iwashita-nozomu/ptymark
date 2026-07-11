use crate::artifact::{ArtifactFormat, RenderArtifact};
use crate::model::SemanticBlock;
use crate::renderer::RenderError;

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerminalCapabilities {
    pub inline_images: bool,
    pub sixel: bool,
    pub color: bool,
}

impl TerminalCapabilities {
    pub const fn new() -> Self {
        Self {
            inline_images: false,
            sixel: false,
            color: false,
        }
    }

    pub const fn with_inline_images(mut self, enabled: bool) -> Self {
        self.inline_images = enabled;
        self
    }

    pub const fn with_sixel(mut self, enabled: bool) -> Self {
        self.sixel = enabled;
        self
    }

    pub const fn with_color(mut self, enabled: bool) -> Self {
        self.color = enabled;
        self
    }

    pub const fn fingerprint(self) -> u64 {
        (self.inline_images as u64) | ((self.sixel as u64) << 1) | ((self.color as u64) << 2)
    }
}

pub trait ArtifactPresenter: Send {
    fn id(&self) -> &str;
    fn accepted_formats(&self) -> &[ArtifactFormat];
    fn present(
        &mut self,
        artifact: &RenderArtifact,
        source: &SemanticBlock,
        capabilities: TerminalCapabilities,
    ) -> Result<Vec<u8>, RenderError>;
}

impl<T: ArtifactPresenter + ?Sized> ArtifactPresenter for Box<T> {
    fn id(&self) -> &str {
        (**self).id()
    }

    fn accepted_formats(&self) -> &[ArtifactFormat] {
        (**self).accepted_formats()
    }

    fn present(
        &mut self,
        artifact: &RenderArtifact,
        source: &SemanticBlock,
        capabilities: TerminalCapabilities,
    ) -> Result<Vec<u8>, RenderError> {
        (**self).present(artifact, source, capabilities)
    }
}

#[derive(Debug)]
pub struct TerminalTextPresenter {
    accepted: [ArtifactFormat; 3],
}

impl Default for TerminalTextPresenter {
    fn default() -> Self {
        Self {
            accepted: [
                ArtifactFormat::TerminalText,
                ArtifactFormat::Source,
                ArtifactFormat::MathMl,
            ],
        }
    }
}

impl ArtifactPresenter for TerminalTextPresenter {
    fn id(&self) -> &str {
        "terminal/text-v1"
    }

    fn accepted_formats(&self) -> &[ArtifactFormat] {
        &self.accepted
    }

    fn present(
        &mut self,
        artifact: &RenderArtifact,
        _source: &SemanticBlock,
        _capabilities: TerminalCapabilities,
    ) -> Result<Vec<u8>, RenderError> {
        if !self.accepted.contains(&artifact.format) {
            return Err(RenderError::new(format!(
                "presenter `{}` cannot display artifact format `{}`",
                self.id(),
                artifact.format.as_str()
            )));
        }
        Ok(artifact.bytes.clone())
    }
}

#[derive(Debug)]
pub struct SourcePresenter {
    accepted: [ArtifactFormat; 1],
}

impl Default for SourcePresenter {
    fn default() -> Self {
        Self::new()
    }
}

impl SourcePresenter {
    pub fn new() -> Self {
        Self {
            accepted: [ArtifactFormat::Source],
        }
    }
}

impl ArtifactPresenter for SourcePresenter {
    fn id(&self) -> &str {
        "terminal/source-v1"
    }

    fn accepted_formats(&self) -> &[ArtifactFormat] {
        &self.accepted
    }

    fn present(
        &mut self,
        artifact: &RenderArtifact,
        source: &SemanticBlock,
        _capabilities: TerminalCapabilities,
    ) -> Result<Vec<u8>, RenderError> {
        if artifact.format == ArtifactFormat::Source {
            Ok(artifact.bytes.clone())
        } else {
            Ok(source.source().to_vec())
        }
    }
}
