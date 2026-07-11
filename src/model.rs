use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticBlock {
    kind: BlockKind,
    source: Vec<u8>,
    body: Vec<u8>,
}

impl SemanticBlock {
    pub fn new(kind: BlockKind, source: Vec<u8>, body: Vec<u8>) -> Self {
        Self { kind, source, body }
    }

    pub const fn kind(&self) -> BlockKind {
        self.kind
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DisplayMode {
    #[default]
    Transform,
    Bypass,
}
