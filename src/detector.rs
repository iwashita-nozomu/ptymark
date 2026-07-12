use crate::config::DetectionConfig;
use crate::model::{BlockKind, SemanticBlock, StreamItem, push_passthrough};
use std::mem;

pub trait SemanticDetector: Send {
    fn feed(&mut self, input: &[u8]) -> Vec<StreamItem>;
    fn finish(&mut self) -> Vec<StreamItem>;
}

#[derive(Clone, Debug)]
struct OpenRule {
    pattern: Vec<u8>,
    kind: BlockKind,
    closing: Vec<u8>,
}

#[derive(Debug)]
enum State {
    LineStart(Vec<u8>),
    PassthroughLine,
    Block {
        kind: BlockKind,
        closing: Vec<u8>,
        source: Vec<u8>,
        body: Vec<u8>,
        line: Vec<u8>,
    },
}

#[derive(Debug)]
pub struct FencedDetector {
    state: State,
    openers: Vec<OpenRule>,
    max_block_bytes: usize,
}

impl FencedDetector {
    pub fn new(config: &DetectionConfig) -> Self {
        Self {
            state: State::LineStart(Vec::new()),
            openers: build_openers(config),
            max_block_bytes: config.max_block_bytes.max(1),
        }
    }

    fn exact_opener(&self, candidate: &[u8]) -> Option<OpenRule> {
        self.openers
            .iter()
            .find(|rule| rule.pattern == candidate)
            .cloned()
    }

    fn opener_prefix(&self, candidate: &[u8]) -> bool {
        self.openers
            .iter()
            .any(|rule| rule.pattern.starts_with(candidate))
    }
}

impl SemanticDetector for FencedDetector {
    fn feed(&mut self, input: &[u8]) -> Vec<StreamItem> {
        let mut items = Vec::new();

        for &byte in input {
            let state = mem::replace(&mut self.state, State::LineStart(Vec::new()));
            self.state = match state {
                State::LineStart(mut candidate) => {
                    candidate.push(byte);
                    if let Some(rule) = self.exact_opener(&candidate) {
                        State::Block {
                            kind: rule.kind,
                            closing: rule.closing,
                            source: candidate,
                            body: Vec::new(),
                            line: Vec::new(),
                        }
                    } else if self.opener_prefix(&candidate) {
                        State::LineStart(candidate)
                    } else {
                        push_passthrough(&mut items, candidate);
                        if byte == b'\n' {
                            State::LineStart(Vec::new())
                        } else {
                            State::PassthroughLine
                        }
                    }
                }
                State::PassthroughLine => {
                    push_passthrough(&mut items, [byte]);
                    if byte == b'\n' {
                        State::LineStart(Vec::new())
                    } else {
                        State::PassthroughLine
                    }
                }
                State::Block {
                    kind,
                    closing,
                    mut source,
                    mut body,
                    mut line,
                } => {
                    source.push(byte);
                    line.push(byte);

                    if source.len() > self.max_block_bytes {
                        push_passthrough(&mut items, source);
                        if byte == b'\n' {
                            State::LineStart(Vec::new())
                        } else {
                            State::PassthroughLine
                        }
                    } else if byte == b'\n' {
                        if strip_eol(&line) == closing {
                            items
                                .push(StreamItem::Semantic(SemanticBlock::new(kind, source, body)));
                            State::LineStart(Vec::new())
                        } else {
                            body.extend_from_slice(&line);
                            line.clear();
                            State::Block {
                                kind,
                                closing,
                                source,
                                body,
                                line,
                            }
                        }
                    } else {
                        State::Block {
                            kind,
                            closing,
                            source,
                            body,
                            line,
                        }
                    }
                }
            };
        }

        items
    }

    fn finish(&mut self) -> Vec<StreamItem> {
        let state = mem::replace(&mut self.state, State::LineStart(Vec::new()));
        let mut items = Vec::new();
        match state {
            State::LineStart(candidate) => push_passthrough(&mut items, candidate),
            State::PassthroughLine => {}
            State::Block { source, .. } => push_passthrough(&mut items, source),
        }
        items
    }
}

#[derive(Debug, Default)]
pub struct PassthroughDetector;

impl SemanticDetector for PassthroughDetector {
    fn feed(&mut self, input: &[u8]) -> Vec<StreamItem> {
        if input.is_empty() {
            Vec::new()
        } else {
            vec![StreamItem::Passthrough(input.to_vec())]
        }
    }

    fn finish(&mut self) -> Vec<StreamItem> {
        Vec::new()
    }
}

fn strip_eol(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

fn add_rule(openers: &mut Vec<OpenRule>, opening: &str, kind: BlockKind, closing: &str) {
    for ending in ["\n", "\r\n"] {
        openers.push(OpenRule {
            pattern: format!("{opening}{ending}").into_bytes(),
            kind,
            closing: closing.as_bytes().to_vec(),
        });
    }
}

fn build_openers(config: &DetectionConfig) -> Vec<OpenRule> {
    let mut openers = Vec::new();
    if config.math {
        add_rule(&mut openers, "$$", BlockKind::Math, "$$");
        for name in ["math", "latex", "tex"] {
            add_rule(&mut openers, &format!("```{name}"), BlockKind::Math, "```");
        }
    }
    if config.mermaid {
        add_rule(&mut openers, "```mermaid", BlockKind::Mermaid, "```");
    }
    openers
}

#[cfg(test)]
mod tests {
    use super::{FencedDetector, SemanticDetector};
    use crate::config::DetectionConfig;
    use crate::model::{BlockKind, StreamItem};

    fn flatten(items: Vec<StreamItem>) -> Vec<u8> {
        items
            .into_iter()
            .flat_map(|item| match item {
                StreamItem::Passthrough(bytes) => bytes,
                StreamItem::Semantic(block) => block.source().to_vec(),
            })
            .collect()
    }

    #[test]
    fn detects_mermaid_across_single_byte_chunks() {
        let source = b"before\n```mermaid\nA --> B\n```\nafter\n";
        let mut detector = FencedDetector::new(&DetectionConfig::default());
        let mut items = Vec::new();
        for byte in source {
            items.extend(detector.feed(&[*byte]));
        }
        items.extend(detector.finish());

        assert_eq!(flatten(items.clone()), source);
        let blocks: Vec<_> = items
            .into_iter()
            .filter_map(|item| match item {
                StreamItem::Semantic(block) => Some(block),
                StreamItem::Passthrough(_) => None,
            })
            .collect();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind(), BlockKind::Mermaid);
        assert_eq!(blocks[0].body(), b"A --> B\n");
    }

    #[test]
    fn incomplete_block_returns_to_source() {
        let source = b"```mermaid\nA --> B\n";
        let mut detector = FencedDetector::new(&DetectionConfig::default());
        let mut items = detector.feed(source);
        items.extend(detector.finish());
        assert_eq!(flatten(items), source);
    }

    #[test]
    fn oversized_block_returns_to_source() {
        let config = DetectionConfig {
            max_block_bytes: 12,
            ..DetectionConfig::default()
        };
        let source = b"$$\n123456789\n$$\n";
        let mut detector = FencedDetector::new(&config);
        let mut items = detector.feed(source);
        items.extend(detector.finish());
        assert_eq!(flatten(items), source);
    }
}
