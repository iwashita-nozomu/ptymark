use crate::model::BlockKind;
use crate::ui::LayoutSensitivity;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ArtifactFormat {
    TerminalText,
    Svg,
    Png,
    MathMl,
    Source,
}

impl ArtifactFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TerminalText => "terminal-text",
            Self::Svg => "svg",
            Self::Png => "png",
            Self::MathMl => "mathml",
            Self::Source => "source",
        }
    }

    pub const fn media_type(self) -> &'static str {
        match self {
            Self::TerminalText => "text/plain; charset=utf-8",
            Self::Svg => "image/svg+xml",
            Self::Png => "image/png",
            Self::MathMl => "application/mathml+xml",
            Self::Source => "text/plain; charset=utf-8",
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EngineIdentity {
    pub id: String,
    pub version: String,
}

impl EngineIdentity {
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderArtifact {
    pub format: ArtifactFormat,
    pub bytes: Vec<u8>,
    pub engine: EngineIdentity,
    pub block_kind: BlockKind,
    pub layout_sensitivity: LayoutSensitivity,
    pub cacheable: bool,
    pub diagnostics: Vec<String>,
}

impl RenderArtifact {
    pub fn new(
        format: ArtifactFormat,
        bytes: Vec<u8>,
        engine: EngineIdentity,
        block_kind: BlockKind,
        layout_sensitivity: LayoutSensitivity,
    ) -> Self {
        Self {
            format,
            bytes,
            engine,
            block_kind,
            layout_sensitivity,
            cacheable: true,
            diagnostics: Vec::new(),
        }
    }

    pub fn not_cacheable(mut self) -> Self {
        self.cacheable = false;
        self
    }

    pub fn with_diagnostic(mut self, diagnostic: impl Into<String>) -> Self {
        self.diagnostics.push(diagnostic.into());
        self
    }
}
