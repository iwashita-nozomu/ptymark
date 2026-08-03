use crate::limits::MAX_TERMINAL_CONTROL_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputSegment {
    SafeText(Vec<u8>),
    RawTerminalBytes(Vec<u8>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AlternateScreenEvent {
    Enter,
    Leave,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ParserEvent {
    alternate_screen: Option<AlternateScreenEvent>,
    overflowed: bool,
}

#[derive(Clone, Debug, Default)]
enum ParserState {
    #[default]
    Ground,
    Escape,
    Csi(Vec<u8>),
    Osc {
        escaped: bool,
    },
    String {
        escaped: bool,
    },
}

#[derive(Clone, Debug, Default)]
struct ControlParser {
    state: ParserState,
}

impl ControlParser {
    fn is_ground(&self) -> bool {
        matches!(self.state, ParserState::Ground)
    }

    fn feed(&mut self, byte: u8) -> ParserEvent {
        match &mut self.state {
            ParserState::Ground => {
                if byte == 0x1b {
                    self.state = ParserState::Escape;
                }
                ParserEvent::default()
            }
            ParserState::Escape => {
                match byte {
                    b'[' => self.state = ParserState::Csi(vec![0x1b, b'[']),
                    b']' => self.state = ParserState::Osc { escaped: false },
                    b'P' | b'_' | b'^' | b'X' => {
                        self.state = ParserState::String { escaped: false };
                    }
                    0x1b => self.state = ParserState::Escape,
                    _ => self.state = ParserState::Ground,
                }
                ParserEvent::default()
            }
            ParserState::Csi(bytes) => {
                if bytes.len() >= MAX_TERMINAL_CONTROL_BYTES {
                    // The complete sequence cannot be classified within the
                    // internal safety bound. Stop retaining bytes and ask the
                    // gate to remain raw for the rest of this session.
                    self.state = ParserState::Ground;
                    return ParserEvent {
                        overflowed: true,
                        alternate_screen: None,
                    };
                }
                bytes.push(byte);
                if (0x40..=0x7e).contains(&byte) {
                    let alternate_screen = alternate_screen_event(bytes);
                    self.state = ParserState::Ground;
                    ParserEvent {
                        alternate_screen,
                        overflowed: false,
                    }
                } else {
                    ParserEvent::default()
                }
            }
            ParserState::Osc { escaped } => {
                if byte == 0x07 || (*escaped && byte == b'\\') {
                    self.state = ParserState::Ground;
                } else {
                    *escaped = byte == 0x1b;
                }
                ParserEvent::default()
            }
            ParserState::String { escaped } => {
                if *escaped && byte == b'\\' {
                    self.state = ParserState::Ground;
                } else {
                    *escaped = byte == 0x1b;
                }
                ParserEvent::default()
            }
        }
    }
}

fn alternate_screen_event(sequence: &[u8]) -> Option<AlternateScreenEvent> {
    if sequence.len() < 4 || sequence.get(..2) != Some(&[0x1b, b'[']) {
        return None;
    }
    let final_byte = *sequence.last()?;
    if !matches!(final_byte, b'h' | b'l') {
        return None;
    }
    let parameters = sequence.get(2..sequence.len() - 1)?.strip_prefix(b"?")?;
    let alternate = parameters
        .split(|byte| *byte == b';')
        .any(|parameter| matches!(parameter, b"47" | b"1047" | b"1049"));
    if !alternate {
        return None;
    }
    Some(if final_byte == b'h' {
        AlternateScreenEvent::Enter
    } else {
        AlternateScreenEvent::Leave
    })
}

fn is_unsafe_control(byte: u8) -> bool {
    byte == 0x1b || (byte < 0x20 && !matches!(byte, b'\n' | b'\t')) || byte == 0x7f
}

fn push_segment(segments: &mut Vec<OutputSegment>, raw: bool, byte: u8) {
    match segments.last_mut() {
        Some(OutputSegment::RawTerminalBytes(bytes)) if raw => bytes.push(byte),
        Some(OutputSegment::SafeText(bytes)) if !raw => bytes.push(byte),
        _ if raw => segments.push(OutputSegment::RawTerminalBytes(vec![byte])),
        _ => segments.push(OutputSegment::SafeText(vec![byte])),
    }
}

#[derive(Clone, Debug, Default)]
pub struct TerminalOutputGate {
    raw_until_newline: bool,
    alternate_screen: bool,
    pending_carriage_return: bool,
    parser_fail_closed: bool,
    parser: ControlParser,
}

impl TerminalOutputGate {
    pub fn feed(&mut self, input: &[u8]) -> Vec<OutputSegment> {
        let mut segments = Vec::new();

        for &byte in input {
            if self.pending_carriage_return {
                self.pending_carriage_return = false;
                if byte == b'\n'
                    && !self.alternate_screen
                    && !self.raw_until_newline
                    && !self.parser_fail_closed
                    && self.parser.is_ground()
                {
                    push_segment(&mut segments, false, b'\r');
                    push_segment(&mut segments, false, b'\n');
                    continue;
                }
                self.process_byte(b'\r', &mut segments);
            }

            if byte == b'\r'
                && !self.alternate_screen
                && !self.raw_until_newline
                && !self.parser_fail_closed
                && self.parser.is_ground()
            {
                self.pending_carriage_return = true;
            } else {
                self.process_byte(byte, &mut segments);
            }
        }

        segments
    }

    pub fn finish(&mut self) -> Vec<OutputSegment> {
        let mut segments = Vec::new();
        if self.pending_carriage_return {
            self.pending_carriage_return = false;
            self.process_byte(b'\r', &mut segments);
        }
        segments
    }

    pub const fn is_alternate_screen(&self) -> bool {
        self.alternate_screen
    }

    fn process_byte(&mut self, byte: u8, segments: &mut Vec<OutputSegment>) {
        let parser_active = !self.parser.is_ground();
        let unsafe_control = is_unsafe_control(byte);
        let raw = self.parser_fail_closed
            || self.alternate_screen
            || self.raw_until_newline
            || parser_active
            || unsafe_control;
        push_segment(segments, raw, byte);

        if unsafe_control {
            self.raw_until_newline = true;
        }

        let event = self.parser.feed(byte);
        if event.overflowed {
            self.parser_fail_closed = true;
            self.raw_until_newline = true;
        }
        if let Some(event) = event.alternate_screen {
            match event {
                AlternateScreenEvent::Enter => {
                    self.alternate_screen = true;
                    self.raw_until_newline = true;
                }
                AlternateScreenEvent::Leave => {
                    self.alternate_screen = false;
                    self.raw_until_newline = true;
                }
            }
        }

        if byte == b'\n'
            && !self.parser_fail_closed
            && !self.alternate_screen
            && self.parser.is_ground()
        {
            self.raw_until_newline = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OutputSegment, TerminalOutputGate};
    use crate::limits::MAX_TERMINAL_CONTROL_BYTES;

    fn flatten(segments: Vec<OutputSegment>) -> Vec<u8> {
        segments
            .into_iter()
            .flat_map(|segment| match segment {
                OutputSegment::SafeText(bytes) | OutputSegment::RawTerminalBytes(bytes) => bytes,
            })
            .collect()
    }

    #[test]
    fn control_sequences_are_byte_exact() {
        let source = b"plain\x1b[31m red\x1b[0m\n\x1b]8;;https://example.com\x07link\x1b]8;;\x07\n";
        let mut gate = TerminalOutputGate::default();
        let mut output = Vec::new();
        for byte in source {
            output.extend(flatten(gate.feed(&[*byte])));
        }
        output.extend(flatten(gate.finish()));
        assert_eq!(output, source);
    }

    #[test]
    fn alternate_screen_is_never_safe_text() {
        let source = b"\x1b[?1049h$$\nE = mc^2\n$$\n\x1b[?1049lplain\n";
        let mut gate = TerminalOutputGate::default();
        let segments = gate.feed(source);
        assert_eq!(flatten(segments), source);
        assert!(!gate.is_alternate_screen());
    }

    #[test]
    fn crlf_is_safe_text_even_when_split_across_chunks() {
        let source = b"$$\r\nE = mc^2\r\n$$\r\n";
        let mut gate = TerminalOutputGate::default();
        let mut segments = Vec::new();
        for byte in source {
            segments.extend(gate.feed(&[*byte]));
        }
        segments.extend(gate.finish());
        assert!(
            segments
                .iter()
                .all(|segment| matches!(segment, OutputSegment::SafeText(_)))
        );
        assert_eq!(flatten(segments), source);
    }

    #[test]
    fn bare_carriage_return_keeps_progress_output_raw_until_newline() {
        let mut gate = TerminalOutputGate::default();
        let segments = gate.feed(b"10%\r20%\nplain\n");
        assert_eq!(
            segments,
            vec![
                OutputSegment::SafeText(b"10%".to_vec()),
                OutputSegment::RawTerminalBytes(b"\r20%\n".to_vec()),
                OutputSegment::SafeText(b"plain\n".to_vec()),
            ]
        );
    }

    #[test]
    fn trailing_carriage_return_is_flushed_as_raw() {
        let mut gate = TerminalOutputGate::default();
        assert_eq!(
            gate.feed(b"progress\r"),
            vec![OutputSegment::SafeText(b"progress".to_vec())]
        );
        assert_eq!(
            gate.finish(),
            vec![OutputSegment::RawTerminalBytes(b"\r".to_vec())]
        );
    }

    #[test]
    fn oversized_csi_is_bounded_and_fails_closed() {
        let mut source = vec![0x1b, b'['];
        source.extend(std::iter::repeat_n(b'0', MAX_TERMINAL_CONTROL_BYTES));
        source.extend_from_slice(b"m\n$$\nE = mc^2\n$$\n");

        let mut gate = TerminalOutputGate::default();
        let segments = gate.feed(&source);
        assert_eq!(flatten(segments.clone()), source);
        assert!(
            segments
                .iter()
                .all(|segment| matches!(segment, OutputSegment::RawTerminalBytes(_)))
        );
    }

    #[test]
    fn incomplete_csi_at_eof_is_byte_exact() {
        let source = b"before\x1b[123";
        let mut gate = TerminalOutputGate::default();
        let mut output = flatten(gate.feed(source));
        output.extend(flatten(gate.finish()));
        assert_eq!(output, source);
    }
}
