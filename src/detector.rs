use crate::model::{BlockKind, SemanticBlock, StreamItem};
use std::collections::BTreeMap;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FencedDetectorOptions {
    pub max_buffer_bytes: usize,
    pub max_line_bytes: usize,
    pub mermaid: bool,
    pub block_math: bool,
    pub mermaid_fences: Vec<String>,
    pub math_fences: Vec<String>,
}

impl Default for FencedDetectorOptions {
    fn default() -> Self {
        Self {
            max_buffer_bytes: 1024 * 1024,
            max_line_bytes: 64 * 1024,
            mermaid: true,
            block_math: true,
            mermaid_fences: vec!["mermaid".to_owned()],
            math_fences: vec!["math".to_owned(), "latex".to_owned(), "tex".to_owned()],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClosingBoundary {
    CodeFence,
    DollarFence,
}

#[derive(Clone, Debug)]
struct Opener {
    bytes: Vec<u8>,
    kind: BlockKind,
    closing: ClosingBoundary,
}

#[derive(Debug)]
enum DetectorState {
    LineStart {
        candidate: Vec<u8>,
    },
    PassthroughLine,
    Buffering {
        kind: BlockKind,
        closing: ClosingBoundary,
        source: Vec<u8>,
        body: Vec<u8>,
        pending_line: Vec<u8>,
    },
}

#[derive(Debug)]
pub struct FencedDetector {
    state: DetectorState,
    options: FencedDetectorOptions,
    openers: Vec<Opener>,
}

impl FencedDetector {
    pub fn new(max_buffer_bytes: usize) -> Self {
        let max_buffer_bytes = max_buffer_bytes.max(1);
        Self::with_options(FencedDetectorOptions {
            max_buffer_bytes,
            max_line_bytes: (64 * 1024).min(max_buffer_bytes),
            ..FencedDetectorOptions::default()
        })
    }

    pub fn with_options(mut options: FencedDetectorOptions) -> Self {
        options.max_buffer_bytes = options.max_buffer_bytes.max(1);
        options.max_line_bytes = options.max_line_bytes.max(1).min(options.max_buffer_bytes);
        let openers = build_openers(&options);
        Self {
            state: DetectorState::LineStart {
                candidate: Vec::new(),
            },
            options,
            openers,
        }
    }

    pub const fn options(&self) -> &FencedDetectorOptions {
        &self.options
    }

    fn opener(&self, candidate: &[u8]) -> Option<(BlockKind, ClosingBoundary)> {
        self.openers
            .iter()
            .find(|opener| opener.bytes == candidate)
            .map(|opener| (opener.kind, opener.closing))
    }

    fn opener_is_still_possible(&self, candidate: &[u8]) -> bool {
        self.openers
            .iter()
            .any(|opener| opener.bytes.starts_with(candidate))
    }
}

fn valid_alias(alias: &str) -> bool {
    !alias.is_empty()
        && alias
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn build_openers(options: &FencedDetectorOptions) -> Vec<Opener> {
    let mut patterns: BTreeMap<Vec<u8>, Option<(BlockKind, ClosingBoundary)>> = BTreeMap::new();

    if options.block_math {
        insert_opener_patterns(
            &mut patterns,
            b"$$",
            BlockKind::Math,
            ClosingBoundary::DollarFence,
        );
        for alias in &options.math_fences {
            if valid_alias(alias) {
                let token = format!("```{alias}");
                insert_opener_patterns(
                    &mut patterns,
                    token.as_bytes(),
                    BlockKind::Math,
                    ClosingBoundary::CodeFence,
                );
            }
        }
    }

    if options.mermaid {
        for alias in &options.mermaid_fences {
            if valid_alias(alias) {
                let token = format!("```{alias}");
                insert_opener_patterns(
                    &mut patterns,
                    token.as_bytes(),
                    BlockKind::Mermaid,
                    ClosingBoundary::CodeFence,
                );
            }
        }
    }

    patterns
        .into_iter()
        .filter_map(|(bytes, value)| {
            value.map(|(kind, closing)| Opener {
                bytes,
                kind,
                closing,
            })
        })
        .collect()
}

fn insert_opener_patterns(
    patterns: &mut BTreeMap<Vec<u8>, Option<(BlockKind, ClosingBoundary)>>,
    token: &[u8],
    kind: BlockKind,
    closing: ClosingBoundary,
) {
    for leading_spaces in 0..=3 {
        for line_ending in [b"\n".as_slice(), b"\r\n".as_slice()] {
            let mut pattern = vec![b' '; leading_spaces];
            pattern.extend_from_slice(token);
            pattern.extend_from_slice(line_ending);
            match patterns.get_mut(&pattern) {
                Some(existing) if *existing != Some((kind, closing)) => *existing = None,
                Some(_) => {}
                None => {
                    patterns.insert(pattern, Some((kind, closing)));
                }
            }
        }
    }
}

fn closing_line(boundary: ClosingBoundary, line: &[u8]) -> bool {
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
    match boundary {
        ClosingBoundary::CodeFence => line == b"```",
        ClosingBoundary::DollarFence => line == b"$$",
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
                    if candidate.len() > self.options.max_line_bytes {
                        push_passthrough(&mut output, &candidate);
                        if byte == b'\n' {
                            DetectorState::LineStart {
                                candidate: Vec::new(),
                            }
                        } else {
                            DetectorState::PassthroughLine
                        }
                    } else if let Some((kind, closing)) = self.opener(&candidate) {
                        DetectorState::Buffering {
                            kind,
                            closing,
                            source: candidate,
                            body: Vec::new(),
                            pending_line: Vec::new(),
                        }
                    } else if self.opener_is_still_possible(&candidate) {
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
                    closing,
                    mut source,
                    mut body,
                    mut pending_line,
                } => {
                    source.push(byte);
                    pending_line.push(byte);

                    if source.len() > self.options.max_buffer_bytes
                        || pending_line.len() > self.options.max_line_bytes
                    {
                        push_passthrough(&mut output, &source);
                        if byte == b'\n' {
                            DetectorState::LineStart {
                                candidate: Vec::new(),
                            }
                        } else {
                            DetectorState::PassthroughLine
                        }
                    } else if byte == b'\n' {
                        if closing_line(closing, &pending_line) {
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
                                closing,
                                source,
                                body,
                                pending_line,
                            }
                        }
                    } else {
                        DetectorState::Buffering {
                            kind,
                            closing,
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
    use super::{FencedDetector, FencedDetectorOptions, SemanticDetector};
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
    fn configured_math_fence_alias_is_explicit_and_chunk_safe() {
        let options = FencedDetectorOptions {
            math_fences: vec!["latex".to_owned()],
            ..FencedDetectorOptions::default()
        };
        let mut detector = FencedDetector::with_options(options);
        let mut items = detector.feed(b"```la").expect("first feed");
        items.extend(
            detector
                .feed(b"tex\nx^2 + y^2\n```\n")
                .expect("second feed"),
        );
        items.extend(detector.finish().expect("finish"));
        assert!(matches!(
            items.as_slice(),
            [StreamItem::Semantic(block)]
                if block.kind() == BlockKind::Math && block.body() == b"x^2 + y^2\n"
        ));
    }

    #[test]
    fn ambiguous_cross_kind_alias_is_not_detected() {
        let options = FencedDetectorOptions {
            mermaid_fences: vec!["diagram".to_owned()],
            math_fences: vec!["diagram".to_owned()],
            ..FencedDetectorOptions::default()
        };
        let source = b"```diagram\nA --> B\n```\n";
        let mut detector = FencedDetector::with_options(options);
        let mut items = detector.feed(source).expect("feed");
        items.extend(detector.finish().expect("finish"));
        let bytes = items
            .into_iter()
            .flat_map(|item| match item {
                StreamItem::Passthrough(bytes) => bytes,
                StreamItem::Semantic(_) => panic!("ambiguous alias must remain passthrough"),
            })
            .collect::<Vec<_>>();
        assert_eq!(bytes, source);
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

    #[test]
    fn disabling_a_kind_can_only_make_detection_stricter() {
        let source = b"before\n```mermaid\nA --> B\n```\nafter\n";
        let options = FencedDetectorOptions {
            max_buffer_bytes: 1024,
            max_line_bytes: 128,
            mermaid: false,
            ..FencedDetectorOptions::default()
        };
        let mut detector = FencedDetector::with_options(options);
        let mut items = detector.feed(source).expect("feed");
        items.extend(detector.finish().expect("finish"));
        let bytes = items
            .into_iter()
            .flat_map(|item| match item {
                StreamItem::Passthrough(bytes) => bytes,
                StreamItem::Semantic(_) => panic!("disabled kind must remain passthrough"),
            })
            .collect::<Vec<_>>();
        assert_eq!(bytes, source);
    }

    #[test]
    fn line_limit_restores_exact_source() {
        let source = b"$$\n123456789\n$$\n";
        let options = FencedDetectorOptions {
            max_buffer_bytes: 1024,
            max_line_bytes: 4,
            ..FencedDetectorOptions::default()
        };
        let mut detector = FencedDetector::with_options(options);
        let mut items = detector.feed(source).expect("feed");
        items.extend(detector.finish().expect("finish"));
        let bytes = items
            .into_iter()
            .flat_map(|item| match item {
                StreamItem::Passthrough(bytes) => bytes,
                StreamItem::Semantic(_) => panic!("overlong line must remain passthrough"),
            })
            .collect::<Vec<_>>();
        assert_eq!(bytes, source);
    }
}
