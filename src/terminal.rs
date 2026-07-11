#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputSegment {
    SafeText(Vec<u8>),
    RawTerminalBytes(Vec<u8>),
}

pub trait DisplayOutputGate: Send {
    fn feed(&mut self, input: &[u8]) -> Vec<OutputSegment>;
    fn finish(&mut self) -> Vec<OutputSegment>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AlternateEvent {
    Enter,
    Leave,
}

#[derive(Clone, Debug, Default)]
enum ControlState {
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
    state: ControlState,
}

impl ControlParser {
    fn is_ground(&self) -> bool {
        matches!(self.state, ControlState::Ground)
    }

    fn feed(&mut self, byte: u8) -> Option<AlternateEvent> {
        match &mut self.state {
            ControlState::Ground => {
                if byte == 0x1b {
                    self.state = ControlState::Escape;
                }
                None
            }
            ControlState::Escape => {
                match byte {
                    b'[' => self.state = ControlState::Csi(vec![0x1b, b'[']),
                    b']' => self.state = ControlState::Osc { escaped: false },
                    b'P' | b'_' | b'^' | b'X' => {
                        self.state = ControlState::String { escaped: false }
                    }
                    0x1b => self.state = ControlState::Escape,
                    _ => self.state = ControlState::Ground,
                }
                None
            }
            ControlState::Csi(bytes) => {
                bytes.push(byte);
                if (0x40..=0x7e).contains(&byte) {
                    let event = alternate_event(bytes);
                    self.state = ControlState::Ground;
                    event
                } else {
                    None
                }
            }
            ControlState::Osc { escaped } => {
                if byte == 0x07 || (*escaped && byte == b'\\') {
                    self.state = ControlState::Ground;
                } else {
                    *escaped = byte == 0x1b;
                }
                None
            }
            ControlState::String { escaped } => {
                if *escaped && byte == b'\\' {
                    self.state = ControlState::Ground;
                } else {
                    *escaped = byte == 0x1b;
                }
                None
            }
        }
    }
}

fn alternate_event(sequence: &[u8]) -> Option<AlternateEvent> {
    if sequence.len() < 4 || sequence[0..2] != [0x1b, b'['] {
        return None;
    }
    let final_byte = *sequence.last()?;
    if !matches!(final_byte, b'h' | b'l') {
        return None;
    }
    let parameters = &sequence[2..sequence.len() - 1];
    let parameters = parameters.strip_prefix(b"?")?;
    let manages_alternate_screen = parameters
        .split(|byte| *byte == b';')
        .any(|parameter| matches!(parameter, b"47" | b"1047" | b"1049"));
    if !manages_alternate_screen {
        return None;
    }
    Some(if final_byte == b'h' {
        AlternateEvent::Enter
    } else {
        AlternateEvent::Leave
    })
}

#[derive(Clone, Debug, Default)]
pub struct TerminalOutputGate {
    raw_until_newline: bool,
    alternate_screen: bool,
    parser: ControlParser,
}

impl TerminalOutputGate {
    pub const fn is_alternate_screen(&self) -> bool {
        self.alternate_screen
    }
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

impl DisplayOutputGate for TerminalOutputGate {
    fn feed(&mut self, input: &[u8]) -> Vec<OutputSegment> {
        let mut segments = Vec::new();

        for &byte in input {
            let parser_active = !self.parser.is_ground();
            let unsafe_control = is_unsafe_control(byte);
            let raw =
                self.alternate_screen || self.raw_until_newline || parser_active || unsafe_control;
            push_segment(&mut segments, raw, byte);

            if unsafe_control && !matches!(byte, b'\n' | b'\t') {
                self.raw_until_newline = true;
            }

            if let Some(event) = self.parser.feed(byte) {
                match event {
                    AlternateEvent::Enter => {
                        self.alternate_screen = true;
                        self.raw_until_newline = true;
                    }
                    AlternateEvent::Leave => {
                        self.alternate_screen = false;
                        self.raw_until_newline = true;
                    }
                }
            }

            if byte == b'\n' && !self.alternate_screen && self.parser.is_ground() {
                self.raw_until_newline = false;
            }
        }

        segments
    }

    fn finish(&mut self) -> Vec<OutputSegment> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{DisplayOutputGate, OutputSegment, TerminalOutputGate};

    fn flatten(segments: Vec<OutputSegment>) -> Vec<u8> {
        segments
            .into_iter()
            .flat_map(|segment| match segment {
                OutputSegment::SafeText(bytes) | OutputSegment::RawTerminalBytes(bytes) => bytes,
            })
            .collect()
    }

    #[test]
    fn gate_is_byte_exact_for_terminal_sequences() {
        let source = b"plain\x1b[31m red\x1b[0m\n\x1b]8;;https://example.com\x07link\x1b]8;;\x07\n";
        let mut gate = TerminalOutputGate::default();
        let mut output = Vec::new();
        for chunk in source.chunks(1) {
            output.extend(flatten(gate.feed(chunk)));
        }
        output.extend(flatten(gate.finish()));
        assert_eq!(output, source);
    }

    #[test]
    fn alternate_screen_is_raw_until_safe_line_boundary() {
        let source = b"\x1b[?1049h$$\nE = mc^2\n$$\n\x1b[?1049lafter\nplain\n";
        let mut gate = TerminalOutputGate::default();
        let segments = gate.feed(source);
        assert_eq!(flatten(segments), source);
        assert!(!gate.is_alternate_screen());
    }
}
