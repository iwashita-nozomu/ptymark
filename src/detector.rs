use crate::model::{BlockKind, SemanticBlock, StreamItem};
use std::error::Error;
use std::fmt;
use std::mem;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectError {
    message: String,
}

impl DetectError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for DetectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for DetectError {}

pub trait SemanticDetector: Send {
    fn feed(&mut self, input: &[u8]) -> Result<Vec<StreamItem>, DetectError>;
    fn finish(&mut self) -> Result<Vec<StreamItem>, DetectError>;
}

impl<T: SemanticDetector + ?Sized> SemanticDetector for Box<T> {
    fn feed(&mut self, input: &[u8]) -> Result<Vec<StreamItem>, DetectError> {
        (**self).feed(input)
    }

    fn finish(&mut self) -> Result<Vec<StreamItem>, DetectError> {
        (**self).finish()
    }
}

#[derive(Debug)]
enum DetectorState {
    LineStart {
        candidate: Vec<u8>,
    },
    PassthroughLine,
    Buffering {
        kind: BlockKind,
        source: Vec<u8>,
        body: Vec<u8>,
        pending_line: Vec<u8>,
    },
}

#[derive(Debug)]
pub struct FencedDetector {
    state: DetectorState,
    max_buffer_bytes: usize,
}

impl FencedDetector {
    pub fn new(max_buffer_bytes: usize) -> Self {
        Self {
            state: DetectorState::LineStart {
                candidate: Vec::new(),
            },
            max_buffer_bytes: max_buffer_bytes.max(1),
        }
    }
}

const OPENERS: &[(&[u8], BlockKind)] = &[
    (b"$$\n", BlockKind::Math),
    (b"$$\r\n", BlockKind::Math),
    (b" $$\n", BlockKind::Math),
    (b" $$\r\n", BlockKind::Math),
    (b"  $$\n", BlockKind::Math),
    (b"  $$\r\n", BlockKind::Math),
    (b"   $$\n", BlockKind::Math),
    (b"   $$\r\n", BlockKind::Math),
    (b"```mermaid\n", BlockKind::Mermaid),
    (b"```mermaid\r\n", BlockKind::Mermaid),
    (b" ```mermaid\n", BlockKind::Mermaid),
    (b" ```mermaid\r\n", BlockKind::Mermaid),
    (b"  ```mermaid\n", BlockKind::Mermaid),
    (b"  ```mermaid\r\n", BlockKind::Mermaid),
    (b"   ```mermaid\n", BlockKind::Mermaid),
    (b"   ```mermaid\r\n", BlockKind::Mermaid),
];

fn opener_kind(candidate: &[u8]) -> Option<BlockKind> {
    OPENERS
        .iter()
        .find_map(|(pattern, kind)| (*pattern == candidate).then_some(*kind))
}

fn opener_is_still_possible(candidate: &[u8]) -> bool {
    OPENERS
        .iter()
        .any(|(pattern, _)| pattern.starts_with(candidate))
}

fn closing_line(kind: BlockKind, line: &[u8]) -> bool {
    let mut line = line;
    if let Some(stripped) = line.strip_suffix(b"\n") {
        line = stripped;
    }
    if let Some(stripped) = line.strip_suffix(b"\r") {
        line = stripped;
    }
    let leading_spaces = line.iter().take_while(|byte| **byte == b' ').count();
    if leading_spaces > 3 {
        return false;
    }
    let line = &line[leading_spaces..];
    match kind {
        BlockKind::Math => line == b"$$",
        BlockKind::Mermaid => line == b"```",
    }
}

fn push_passthrough(output: &mut Vec<StreamItem>, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    match output.last_mut() {
        Some(StreamItem::Passthrough(existing)) => existing.extend_from_slice(bytes),
        _ => output.push(StreamItem::Passthrough(bytes.to_vec())),
    }
}

impl SemanticDetector for FencedDetector {
    fn feed(&mut self, input: &[u8]) -> Result<Vec<StreamItem>, DetectError> {
        let mut output = Vec::new();

        for &byte in input {
            let state = mem::replace(
                &mut self.state,
                DetectorState::LineStart {
                    candidate: Vec::new(),
                },
            );

            self.state = match state {
                DetectorState::LineStart { mut candidate } => {
                    candidate.push(byte);
                    if candidate.len() > self.max_buffer_bytes {
                        push_passthrough(&mut output, &candidate);
                        if byte == b'\n' {
                            DetectorState::LineStart {
                                candidate: Vec::new(),
                            }
                        } else {
                            DetectorState::PassthroughLine
                        }
                    } else if let Some(kind) = opener_kind(&candidate) {
                        DetectorState::Buffering {
                            kind,
                            source: candidate,
                            body: Vec::new(),
                            pending_line: Vec::new(),
                        }
                    } else if opener_is_still_possible(&candidate) {
                        DetectorState::LineStart { candidate }
                    } else {
                        push_passthrough(&mut output, &candidate);
                        if byte == b'\n' {
                            DetectorState::LineStart {
                                candidate: Vec::new(),
                            }
                        } else {
                            DetectorState::PassthroughLine
                        }
                    }
                }
                DetectorState::PassthroughLine => {
                    push_passthrough(&mut output, &[byte]);
                    if byte == b'\n' {
                        DetectorState::LineStart {
                            candidate: Vec::new(),
                        }
                    } else {
                        DetectorState::PassthroughLine
                    }
                }
                DetectorState::Buffering {
                    kind,
                    mut source,
                    mut body,
                    mut pending_line,
                } => {
                    source.push(byte);
                    pending_line.push(byte);

                    if source.len() > self.max_buffer_bytes {
                        push_passthrough(&mut output, &source);
                        if byte == b'\n' {
                            DetectorState::LineStart {
                                candidate: Vec::new(),
                            }
                        } else {
                            DetectorState::PassthroughLine
                        }
                    } else if byte == b'\n' {
                        if closing_line(kind, &pending_line) {
                            output
                                .push(StreamItem::Semantic(SemanticBlock::new(kind, source, body)));
                            DetectorState::LineStart {
                                candidate: Vec::new(),
                            }
                        } else {
                            body.extend_from_slice(&pending_line);
                            pending_line.clear();
                            DetectorState::Buffering {
                                kind,
                                source,
                                body,
                                pending_line,
                            }
                        }
                    } else {
                        DetectorState::Buffering {
                            kind,
                            source,
                            body,
                            pending_line,
                        }
                    }
                }
            };
        }

        Ok(output)
    }

    fn finish(&mut self) -> Result<Vec<StreamItem>, DetectError> {
        let state = mem::replace(
            &mut self.state,
            DetectorState::LineStart {
                candidate: Vec::new(),
            },
        );
        let mut output = Vec::new();
        match state {
            DetectorState::LineStart { candidate } => {
                push_passthrough(&mut output, &candidate);
            }
            DetectorState::PassthroughLine => {}
            DetectorState::Buffering { source, .. } => {
                push_passthrough(&mut output, &source);
            }
        }
        Ok(output)
    }
}

#[derive(Debug, Default)]
pub struct PassthroughDetector;

impl SemanticDetector for PassthroughDetector {
    fn feed(&mut self, input: &[u8]) -> Result<Vec<StreamItem>, DetectError> {
        Ok(vec![StreamItem::Passthrough(input.to_vec())])
    }

    fn finish(&mut self) -> Result<Vec<StreamItem>, DetectError> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::{FencedDetector, SemanticDetector};
    use crate::model::{BlockKind, StreamItem};

    #[test]
    fn detects_mermaid_across_chunk_boundaries() {
        let mut detector = FencedDetector::new(1024);
        let mut items = detector.feed(b"before\n```mer").expect("first feed");
        items.extend(
            detector
                .feed(b"maid\nA --> B\n```\nafter\n")
                .expect("second feed"),
        );
        items.extend(detector.finish().expect("finish"));

        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], StreamItem::Passthrough(bytes) if bytes == b"before\n"));
        assert!(matches!(
            &items[1],
            StreamItem::Semantic(block)
                if block.kind() == BlockKind::Mermaid && block.body() == b"A --> B\n"
        ));
        assert!(matches!(&items[2], StreamItem::Passthrough(bytes) if bytes == b"after\n"));
    }

    #[test]
    fn ordinary_prompt_without_newline_is_forwarded_immediately() {
        let mut detector = FencedDetector::new(1024);
        let items = detector.feed(b"prompt> ").expect("feed");
        assert_eq!(items, vec![StreamItem::Passthrough(b"prompt> ".to_vec())]);
    }

    #[test]
    fn possible_fence_prefix_is_bounded_then_released() {
        let mut detector = FencedDetector::new(1024);
        assert!(detector.feed(b"$").expect("prefix").is_empty());
        assert_eq!(
            detector.feed(b" ").expect("release"),
            vec![StreamItem::Passthrough(b"$ ".to_vec())]
        );
    }

    #[test]
    fn unfinished_block_is_returned_unchanged() {
        let source = b"$$\nE = mc^2\n";
        let mut detector = FencedDetector::new(1024);
        let mut items = detector.feed(source).expect("feed");
        items.extend(detector.finish().expect("finish"));
        assert_eq!(items, vec![StreamItem::Passthrough(source.to_vec())]);
    }

    #[test]
    fn over_limit_input_is_lossless_passthrough() {
        let source = b"```mermaid\nA --> B\n```\n";
        let mut detector = FencedDetector::new(8);
        let mut items = detector.feed(source).expect("feed");
        items.extend(detector.finish().expect("finish"));

        let bytes: Vec<u8> = items
            .into_iter()
            .flat_map(|item| match item {
                StreamItem::Passthrough(bytes) => bytes,
                StreamItem::Semantic(_) => panic!("over-limit input must not be transformed"),
            })
            .collect();
        assert_eq!(bytes, source);
    }
}
