use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BlockKind {
    Math,
    Mermaid,
}

impl BlockKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Math => "math",
            Self::Mermaid => "mermaid",
        }
    }
}

impl fmt::Display for BlockKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The concrete source representation carried by a semantic block.
///
/// `BlockKind` remains the stable renderer role (`math` or `mermaid`). The
/// format is intentionally separate so structured math encodings can reuse the
/// existing math decision, engine, presentation, and fallback contracts.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SemanticFormat {
    Tex,
    OpenMath,
    Mermaid,
}

impl SemanticFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tex => "tex",
            Self::OpenMath => "openmath",
            Self::Mermaid => "mermaid",
        }
    }

    pub const fn default_for(kind: BlockKind) -> Self {
        match kind {
            BlockKind::Math => Self::Tex,
            BlockKind::Mermaid => Self::Mermaid,
        }
    }
}

impl fmt::Display for SemanticFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticBlock {
    kind: BlockKind,
    format: SemanticFormat,
    source: Vec<u8>,
    body: Vec<u8>,
}

impl SemanticBlock {
    pub fn new(kind: BlockKind, source: Vec<u8>, body: Vec<u8>) -> Self {
        Self::with_format(kind, SemanticFormat::default_for(kind), source, body)
    }

    pub fn with_format(
        kind: BlockKind,
        format: SemanticFormat,
        source: Vec<u8>,
        body: Vec<u8>,
    ) -> Self {
        Self {
            kind,
            format,
            source,
            body,
        }
    }

    pub const fn kind(&self) -> BlockKind {
        self.kind
    }

    pub const fn format(&self) -> SemanticFormat {
        self.format
    }

    pub fn source(&self) -> &[u8] {
        &self.source
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamItem {
    Passthrough(Vec<u8>),
    Semantic(SemanticBlock),
}

pub(crate) fn push_passthrough(items: &mut Vec<StreamItem>, bytes: impl IntoIterator<Item = u8>) {
    match items.last_mut() {
        Some(StreamItem::Passthrough(existing)) => existing.extend(bytes),
        _ => items.push(StreamItem::Passthrough(bytes.into_iter().collect())),
    }
}
