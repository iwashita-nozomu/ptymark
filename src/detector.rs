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
enum FenceState {
    Passthrough,
    Buffering {
        kind: BlockKind,
        source: Vec<u8>,
        body: Vec<u8>,
    },
}

#[derive(Debug)]
pub struct FencedDetector {
    pending_line: Vec<u8>,
    state: FenceState,
    max_buffer_bytes: usize,
    passthrough_until_newline: bool,
}

impl FencedDetector {
    pub fn new(max_buffer_bytes: usize) -> Self {
        Self {
            pending_line: Vec::new(),
            state: FenceState::Passthrough,
            max_buffer_bytes: max_buffer_bytes.max(1),
            passthrough_until_newline: false,
        }
    }

    fn process_line(&mut self, line: Vec<u8>, output: &mut Vec<StreamItem>) {
        let trimmed = trim_ascii_line(&line);
        let current = mem::replace(&mut self.state, FenceState::Passthrough);

        match current {
            FenceState::Passthrough => {
                if trimmed == b"```mermaid" {
                    self.state = FenceState::Buffering {
                        kind: BlockKind::Mermaid,
                        source: line,
                        body: Vec::new(),
                    };
                } else if trimmed == b"$$" {
                    self.state = FenceState::Buffering {
                        kind: BlockKind::Math,
                        source: line,
                        body: Vec::new(),
                    };
                } else {
                    output.push(StreamItem::Passthrough(line));
                }
            }
            FenceState::Buffering {
                kind,
                mut source,
                mut body,
            } => {
                if source.len().saturating_add(line.len()) > self.max_buffer_bytes {
                    source.extend_from_slice(&line);
                    output.push(StreamItem::Passthrough(source));
                    return;
                }

                let closes_block = match kind {
                    BlockKind::Mermaid => trimmed == b"```",
                    BlockKind::Math => trimmed == b"$$",
                };

                source.extend_from_slice(&line);
                if closes_block {
                    output.push(StreamItem::Semantic(SemanticBlock::new(
                        kind, source, body,
                    )));
                } else {
                    body.extend_from_slice(&line);
                    self.state = FenceState::Buffering { kind, source, body };
                }
            }
        }
    }

    fn flush_overlong_fragment(
        &mut self,
        fragment: &[u8],
        fragment_ends_line: bool,
        output: &mut Vec<StreamItem>,
    ) {
        let pending = mem::take(&mut self.pending_line);
        let current = mem::replace(&mut self.state, FenceState::Passthrough);
        let mut original = match current {
            FenceState::Passthrough => pending,
            FenceState::Buffering { mut source, .. } => {
                source.extend_from_slice(&pending);
                source
            }
        };
        original.extend_from_slice(fragment);
        if !original.is_empty() {
            output.push(StreamItem::Passthrough(original));
        }
        self.passthrough_until_newline = !fragment_ends_line;
    }
}

impl SemanticDetector for FencedDetector {
    fn feed(&mut self, input: &[u8]) -> Result<Vec<StreamItem>, DetectError> {
        let mut output = Vec::new();
        let mut cursor = 0;

        while cursor < input.len() {
            if self.passthrough_until_newline {
                let remaining = &input[cursor..];
                if let Some(relative_newline) = remaining.iter().position(|byte| *byte == b'\n') {
                    let end = cursor + relative_newline + 1;
                    output.push(StreamItem::Passthrough(input[cursor..end].to_vec()));
                    cursor = end;
                    self.passthrough_until_newline = false;
                } else {
                    output.push(StreamItem::Passthrough(remaining.to_vec()));
                    break;
                }
                continue;
            }

            let remaining = &input[cursor..];
            if let Some(relative_newline) = remaining.iter().position(|byte| *byte == b'\n') {
                let end = cursor + relative_newline + 1;
                let fragment = &input[cursor..end];
                if self.pending_line.len().saturating_add(fragment.len())
                    > self.max_buffer_bytes
                {
                    self.flush_overlong_fragment(fragment, true, &mut output);
                } else {
                    self.pending_line.extend_from_slice(fragment);
                    let line = mem::take(&mut self.pending_line);
                    self.process_line(line, &mut output);
                }
                cursor = end;
            } else {
                if self.pending_line.len().saturating_add(remaining.len())
                    > self.max_buffer_bytes
                {
                    self.flush_overlong_fragment(remaining, false, &mut output);
                } else {
                    self.pending_line.extend_from_slice(remaining);
                }
                break;
            }
        }

        Ok(output)
    }

    fn finish(&mut self) -> Result<Vec<StreamItem>, DetectError> {
        let mut output = Vec::new();

        if !self.pending_line.is_empty() {
            let line = mem::take(&mut self.pending_line);
            self.process_line(line, &mut output);
        }

        if let FenceState::Buffering { source, .. } =
            mem::replace(&mut self.state, FenceState::Passthrough)
        {
            output.push(StreamItem::Passthrough(source));
        }

        self.passthrough_until_newline = false;
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

fn trim_ascii_line(mut line: &[u8]) -> &[u8] {
    while matches!(line.last(), Some(b'\n') | Some(b'\r')) {
        line = &line[..line.len() - 1];
    }

    while matches!(line.first(), Some(byte) if byte.is_ascii_whitespace()) {
        line = &line[1..];
    }

    while matches!(line.last(), Some(byte) if byte.is_ascii_whitespace()) {
        line = &line[..line.len() - 1];
    }

    line
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
