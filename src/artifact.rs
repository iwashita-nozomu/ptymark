use crate::model::BlockKind;
use crate::ui::LayoutSensitivity;
use std::error::Error;
use std::fmt;

#[non_exhaustive]
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
pub struct ArtifactExpectation<'a> {
    pub engine: &'a EngineIdentity,
    pub block_kind: BlockKind,
    pub format: ArtifactFormat,
    pub layout_sensitivity: LayoutSensitivity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactValidationError {
    message: String,
}

impl ArtifactValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ArtifactValidationError {}

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

    pub fn validate(
        &self,
        expectation: &ArtifactExpectation<'_>,
    ) -> Result<(), ArtifactValidationError> {
        if &self.engine != expectation.engine {
            return Err(ArtifactValidationError::new(format!(
                "artifact engine identity is {}@{}, expected {}@{}",
                self.engine.id,
                self.engine.version,
                expectation.engine.id,
                expectation.engine.version
            )));
        }
        if self.block_kind != expectation.block_kind {
            return Err(ArtifactValidationError::new(format!(
                "artifact semantic kind is `{}`, expected `{}`",
                self.block_kind, expectation.block_kind
            )));
        }
        if self.format != expectation.format {
            return Err(ArtifactValidationError::new(format!(
                "artifact format is `{}`, expected `{}`",
                self.format.as_str(),
                expectation.format.as_str()
            )));
        }
        if self.layout_sensitivity != expectation.layout_sensitivity {
            return Err(ArtifactValidationError::new(format!(
                "artifact layout sensitivity is `{:?}`, expected `{:?}`",
                self.layout_sensitivity, expectation.layout_sensitivity
            )));
        }
        self.validate_payload()
    }

    pub fn validate_payload(&self) -> Result<(), ArtifactValidationError> {
        if self.bytes.is_empty() {
            return Err(ArtifactValidationError::new("artifact payload is empty"));
        }
        match self.format {
            ArtifactFormat::TerminalText | ArtifactFormat::Source => Ok(()),
            ArtifactFormat::Svg => {
                let text = std::str::from_utf8(&self.bytes)
                    .map_err(|_| ArtifactValidationError::new("SVG artifact is not UTF-8"))?;
                if text.contains("<svg") {
                    Ok(())
                } else {
                    Err(ArtifactValidationError::new(
                        "SVG artifact does not contain an `<svg` element",
                    ))
                }
            }
            ArtifactFormat::Png => {
                const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
                if self.bytes.starts_with(PNG_SIGNATURE) {
                    Ok(())
                } else {
                    Err(ArtifactValidationError::new(
                        "PNG artifact has an invalid signature",
                    ))
                }
            }
            ArtifactFormat::MathMl => {
                let text = std::str::from_utf8(&self.bytes)
                    .map_err(|_| ArtifactValidationError::new("MathML artifact is not UTF-8"))?;
                if text.contains("<math") {
                    Ok(())
                } else {
                    Err(ArtifactValidationError::new(
                        "MathML artifact does not contain a `<math` element",
                    ))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactExpectation, ArtifactFormat, EngineIdentity, RenderArtifact,
    };
    use crate::model::BlockKind;
    use crate::ui::LayoutSensitivity;

    fn expectation<'a>(identity: &'a EngineIdentity) -> ArtifactExpectation<'a> {
        ArtifactExpectation {
            engine: identity,
            block_kind: BlockKind::Math,
            format: ArtifactFormat::Svg,
            layout_sensitivity: LayoutSensitivity::Columns,
        }
    }

    #[test]
    fn valid_svg_matches_its_descriptor_contract() {
        let identity = EngineIdentity::new("test/svg", "1");
        let artifact = RenderArtifact::new(
            ArtifactFormat::Svg,
            b"<?xml version=\"1.0\"?><svg/>".to_vec(),
            identity.clone(),
            BlockKind::Math,
            LayoutSensitivity::Columns,
        );
        artifact.validate(&expectation(&identity)).expect("valid");
    }

    #[test]
    fn wrong_kind_and_malformed_payload_are_rejected() {
        let identity = EngineIdentity::new("test/svg", "1");
        let wrong_kind = RenderArtifact::new(
            ArtifactFormat::Svg,
            b"<svg/>".to_vec(),
            identity.clone(),
            BlockKind::Mermaid,
            LayoutSensitivity::Columns,
        );
        assert!(wrong_kind.validate(&expectation(&identity)).is_err());

        let malformed = RenderArtifact::new(
            ArtifactFormat::Svg,
            b"not svg".to_vec(),
            identity.clone(),
            BlockKind::Math,
            LayoutSensitivity::Columns,
        );
        assert!(malformed.validate(&expectation(&identity)).is_err());
    }
}
