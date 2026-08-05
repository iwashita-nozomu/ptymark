use crate::command::ChildCommand;
#[cfg(windows)]
use crate::limits::CONPTY_OUTPUT_DRAIN_GRACE;
use crate::limits::{DEFAULT_PTY_ROWS, RESIZE_POLL_INTERVAL};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, size as terminal_size};
use portable_pty::{
    Child as PtyChild, ChildKiller, CommandBuilder, MasterPty, PtyPair, PtySize, native_pty_system,
};
use std::collections::VecDeque;
use std::env;
use std::io::{self, IsTerminal, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

type SharedMaster = Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>;
type SharedWriter = Arc<Mutex<Option<Box<dyn Write + Send>>>>;
type SharedKiller = Arc<Mutex<Box<dyn ChildKiller + Send + Sync>>>;

/// UTF-8 for the supplementary private-use scalar U+10FFFD. WezTerm sends
/// this code point for the session-local render toggle. Using a private-use
/// scalar avoids delaying the ordinary Escape key while still remaining a
/// bounded sequence that can be recognized across read boundaries.
pub(crate) const RENDER_TOGGLE_SEQUENCE: &[u8] = b"\xf4\x8f\xbf\xbd";

#[derive(Clone, Copy, Debug)]
pub(crate) struct ParentTerminal {
    stdin_is_terminal: bool,
    stdout_is_terminal: bool,
    initial_size: PtySize,
}

impl ParentTerminal {
    pub(crate) fn detect(fallback_columns: u16) -> Self {
        let stdin_is_terminal = io::stdin().is_terminal();
        let stdout_is_terminal = io::stdout().is_terminal();
        Self {
            stdin_is_terminal,
            stdout_is_terminal,
            initial_size: initial_pty_size(fallback_columns, stdout_is_terminal),
        }
    }

    pub(crate) const fn initial_size(self) -> PtySize {
        self.initial_size
    }

    pub(crate) const fn output_is_terminal(self) -> bool {
        self.stdout_is_terminal
    }

    #[cfg(windows)]
    pub(crate) const fn needs_cursor_position_fallback(self) -> bool {
        !(self.stdin_is_terminal && self.stdout_is_terminal)
    }

    pub(crate) fn enter_raw_mode(self) -> Result<RawModeGuard, String> {
        RawModeGuard::acquire(self.stdin_is_terminal && self.stdout_is_terminal)
    }
}

pub(crate) struct RawModeGuard {
    enabled: bool,
}

impl RawModeGuard {
    fn acquire(enabled: bool) -> Result<Self, String> {
        if enabled {
            enable_raw_mode()
                .map_err(|error| format!("cannot enable terminal raw mode: {error}"))?;
        }
        Ok(Self { enabled })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.enabled {
            let _ = disable_raw_mode();
        }
    }
}

pub(crate) struct NativeTerminalSession {
    master: SharedMaster,
    reader: Box<dyn Read + Send>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn PtyChild + Send + Sync>>,
    killer: SharedKiller,
}

impl NativeTerminalSession {
    pub(crate) fn spawn(command: &ChildCommand, size: PtySize) -> Result<Self, String> {
        let system = native_pty_system();
        let PtyPair { master, slave } = system
            .openpty(size)
            .map_err(|error| format!("cannot allocate native PTY: {error}"))?;

        let mut builder = CommandBuilder::new(command.program());
        builder.args(command.arguments());
        builder.env("PTYMARK_ACTIVE", "1");
        if env::var_os("TERM").is_none() {
            builder.env("TERM", "xterm-256color");
        }

        let child = slave.spawn_command(builder).map_err(|error| {
            format!(
                "cannot execute `{}` in native PTY: {error}",
                command.display_name()
            )
        })?;
        drop(slave);

        let reader = master
            .try_clone_reader()
            .map_err(|error| format!("cannot open PTY output reader: {error}"))?;
        let writer = master
            .take_writer()
            .map_err(|error| format!("cannot open PTY input writer: {error}"))?;
        let killer = child.clone_killer();

        Ok(Self {
            master: Arc::new(Mutex::new(Some(master))),
            reader,
            writer: Some(writer),
            child: Some(child),
            killer: Arc::new(Mutex::new(killer)),
        })
    }

    pub(crate) fn output_reader(&mut self) -> &mut (dyn Read + Send) {
        self.reader.as_mut()
    }

    fn take_input_writer(&mut self) -> Result<Box<dyn Write + Send>, String> {
        self.writer
            .take()
            .ok_or_else(|| "PTY input writer was already taken".to_owned())
    }

    fn resize_handle(&self) -> SharedMaster {
        Arc::clone(&self.master)
    }

    #[cfg(all(test, unix))]
    fn resize(&self, size: PtySize) -> Result<(), String> {
        let master = self
            .master
            .lock()
            .map_err(|_| "PTY resize lock was poisoned".to_owned())?;
        master
            .as_ref()
            .ok_or_else(|| "PTY master was already closed".to_owned())?
            .resize(size)
            .map_err(|error| format!("cannot resize child PTY: {error}"))
    }

    pub(crate) fn kill(&mut self) -> Result<(), String> {
        self.kill_handle().kill()
    }

    pub(crate) fn kill_handle(&self) -> NativeSessionKiller {
        NativeSessionKiller {
            killer: Arc::clone(&self.killer),
        }
    }

    #[cfg(all(test, unix))]
    pub(crate) fn wait(&mut self) -> Result<portable_pty::ExitStatus, String> {
        self.child
            .as_mut()
            .ok_or_else(|| "child process is already owned by the exit waiter".to_owned())?
            .wait()
            .map_err(|error| format!("cannot wait for child process: {error}"))
    }

    pub(crate) fn start_exit_waiter(
        &mut self,
    ) -> Result<JoinHandle<Result<portable_pty::ExitStatus, String>>, String> {
        let mut child = self
            .child
            .take()
            .ok_or_else(|| "child process exit waiter was already started".to_owned())?;
        let master = Arc::clone(&self.master);
        thread::Builder::new()
            .name("ptymark-child-wait".to_owned())
            .spawn(move || {
                let result = child
                    .wait()
                    .map_err(|error| format!("cannot wait for child process: {error}"));
                #[cfg(windows)]
                thread::sleep(CONPTY_OUTPUT_DRAIN_GRACE);
                close_shared_master(&master);
                result
            })
            .map_err(|error| format!("cannot start child process exit waiter: {error}"))
    }
}

#[derive(Clone)]
pub(crate) struct NativeSessionKiller {
    killer: SharedKiller,
}

impl NativeSessionKiller {
    pub(crate) fn kill(&self) -> Result<(), String> {
        let mut killer = self
            .killer
            .lock()
            .map_err(|_| "child process killer lock was poisoned".to_owned())?;
        killer
            .kill()
            .map_err(|error| format!("cannot terminate child process: {error}"))
    }
}

pub(crate) struct SessionControl {
    running: Arc<AtomicBool>,
    rendering_enabled: Arc<AtomicBool>,
    input_writer: SharedWriter,
    _input: JoinHandle<()>,
    resize: Option<ResizeMonitor>,
}

impl SessionControl {
    pub(crate) fn start(
        session: &mut NativeTerminalSession,
        parent: ParentTerminal,
    ) -> Result<Self, String> {
        let running = Arc::new(AtomicBool::new(true));
        let rendering_enabled = Arc::new(AtomicBool::new(true));
        let resize = ResizeMonitor::spawn(
            session.resize_handle(),
            Arc::clone(&running),
            parent.initial_size(),
            parent.output_is_terminal(),
        )?;
        let writer = match session.take_input_writer() {
            Ok(writer) => writer,
            Err(error) => {
                running.store(false, Ordering::Release);
                let _ = resize.stop();
                return Err(error);
            }
        };
        let input_writer = Arc::new(Mutex::new(Some(writer)));
        let input = match spawn_input_pump(
            Arc::clone(&input_writer),
            Arc::clone(&running),
            Arc::clone(&rendering_enabled),
        ) {
            Ok(input) => input,
            Err(error) => {
                running.store(false, Ordering::Release);
                close_shared_writer(&input_writer);
                let _ = resize.stop();
                return Err(error);
            }
        };

        Ok(Self {
            running,
            rendering_enabled,
            input_writer,
            _input: input,
            resize: Some(resize),
        })
    }

    #[cfg(windows)]
    pub(crate) fn input_responder(&self) -> InputResponder {
        InputResponder {
            writer: Arc::clone(&self.input_writer),
        }
    }

    pub(crate) fn rendering_enabled(&self) -> bool {
        self.rendering_enabled.load(Ordering::Acquire)
    }

    pub(crate) fn latest_resize(&self) -> Option<PtySize> {
        let resize = self.resize.as_ref()?;
        let mut latest = None;
        while let Ok(size) = resize.receiver.try_recv() {
            latest = Some(size);
        }
        latest
    }

    pub(crate) fn stop(mut self) -> Result<(), String> {
        self.running.store(false, Ordering::Release);
        close_shared_writer(&self.input_writer);
        match self.resize.take() {
            Some(resize) => resize.stop(),
            None => Ok(()),
        }
    }
}

impl Drop for SessionControl {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        close_shared_writer(&self.input_writer);
    }
}

#[cfg(windows)]
#[derive(Clone)]
pub(crate) struct InputResponder {
    writer: SharedWriter,
}

#[cfg(windows)]
impl InputResponder {
    pub(crate) fn send_cursor_position(&self) -> io::Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| io::Error::other("PTY input writer lock was poisoned"))?;
        let writer = writer
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "PTY input is closed"))?;
        writer.write_all(b"\x1b[1;1R")?;
        writer.flush()
    }
}

pub(crate) fn normalize_exit_code(status: &portable_pty::ExitStatus) -> i32 {
    i32::try_from(status.exit_code()).unwrap_or(1)
}

#[derive(Debug, Eq, PartialEq)]
enum SessionInputAction {
    Forward(Vec<u8>),
    ToggleRendering,
}

fn push_forward_action(actions: &mut Vec<SessionInputAction>, byte: u8) {
    match actions.last_mut() {
        Some(SessionInputAction::Forward(bytes)) => bytes.push(byte),
        _ => actions.push(SessionInputAction::Forward(vec![byte])),
    }
}

#[derive(Default)]
struct SessionInputFilter {
    pending: VecDeque<u8>,
}

impl SessionInputFilter {
    fn push(&mut self, input: &[u8], actions: &mut Vec<SessionInputAction>) {
        for &byte in input {
            self.pending.push_back(byte);
            while !self.pending_is_prefix() {
                let byte = self
                    .pending
                    .pop_front()
                    .expect("a non-prefix input candidate is non-empty");
                push_forward_action(actions, byte);
            }
            if self.pending.len() == RENDER_TOGGLE_SEQUENCE.len() {
                self.pending.clear();
                actions.push(SessionInputAction::ToggleRendering);
            }
        }
    }

    fn finish(&mut self, actions: &mut Vec<SessionInputAction>) {
        while let Some(byte) = self.pending.pop_front() {
            push_forward_action(actions, byte);
        }
    }

    fn pending_is_prefix(&self) -> bool {
        self.pending.len() <= RENDER_TOGGLE_SEQUENCE.len()
            && self.pending.iter().copied().eq(RENDER_TOGGLE_SEQUENCE
                .iter()
                .copied()
                .take(self.pending.len()))
    }
}

fn spawn_input_pump(
    writer: SharedWriter,
    running: Arc<AtomicBool>,
    rendering_enabled: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, String> {
    thread::Builder::new()
        .name("ptymark-stdin".to_owned())
        .spawn(move || {
            let stdin = io::stdin();
            let mut input = stdin.lock();
            let mut buffer = [0_u8; 8192];
            let mut filter = SessionInputFilter::default();

            while running.load(Ordering::Acquire) {
                let count = match input.read(&mut buffer) {
                    Ok(0) => {
                        flush_pending_input(&mut filter, &writer, &rendering_enabled);
                        break;
                    }
                    Ok(count) => count,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => {
                        flush_pending_input(&mut filter, &writer, &rendering_enabled);
                        break;
                    }
                };

                let mut actions = Vec::new();
                filter.push(&buffer[..count], &mut actions);
                if !apply_input_actions(&writer, &rendering_enabled, actions) {
                    break;
                }
            }
        })
        .map_err(|error| format!("cannot start terminal input forwarding: {error}"))
}

fn flush_pending_input(
    filter: &mut SessionInputFilter,
    writer: &SharedWriter,
    rendering_enabled: &AtomicBool,
) {
    let mut actions = Vec::new();
    filter.finish(&mut actions);
    let _ = apply_input_actions(writer, rendering_enabled, actions);
}

fn apply_input_actions(
    writer: &SharedWriter,
    rendering_enabled: &AtomicBool,
    actions: Vec<SessionInputAction>,
) -> bool {
    for action in actions {
        match action {
            SessionInputAction::Forward(bytes) => {
                if !forward_input(writer, &bytes) {
                    return false;
                }
            }
            SessionInputAction::ToggleRendering => {
                rendering_enabled.fetch_xor(true, Ordering::AcqRel);
            }
        }
    }
    true
}

fn forward_input(writer: &SharedWriter, bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    let mut writer = match writer.lock() {
        Ok(writer) => writer,
        Err(poisoned) => poisoned.into_inner(),
    };
    let Some(writer) = writer.as_mut() else {
        return false;
    };
    writer.write_all(bytes).is_ok() && writer.flush().is_ok()
}

fn close_shared_writer(writer: &SharedWriter) {
    match writer.lock() {
        Ok(mut writer) => {
            let _ = writer.take();
        }
        Err(poisoned) => {
            let _ = poisoned.into_inner().take();
        }
    }
}

fn close_shared_master(master: &SharedMaster) {
    match master.lock() {
        Ok(mut master) => {
            let _ = master.take();
        }
        Err(poisoned) => {
            let _ = poisoned.into_inner().take();
        }
    }
}

fn initial_pty_size(fallback_columns: u16, terminal_attached: bool) -> PtySize {
    if terminal_attached
        && let Ok((columns, rows)) = terminal_size()
        && columns > 0
        && rows > 0
    {
        return PtySize {
            rows,
            cols: columns,
            pixel_width: 0,
            pixel_height: 0,
        };
    }

    PtySize {
        rows: environment_dimension("LINES").unwrap_or(DEFAULT_PTY_ROWS),
        cols: environment_dimension("COLUMNS").unwrap_or(fallback_columns.max(1)),
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn environment_dimension(name: &str) -> Option<u16> {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| *value > 0)
}

struct ResizeMonitor {
    receiver: Receiver<PtySize>,
    handle: Option<JoinHandle<()>>,
}

impl ResizeMonitor {
    fn spawn(
        master: SharedMaster,
        running: Arc<AtomicBool>,
        initial_size: PtySize,
        enabled: bool,
    ) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel();
        if !enabled {
            return Ok(Self {
                receiver,
                handle: None,
            });
        }

        let handle = thread::Builder::new()
            .name("ptymark-resize".to_owned())
            .spawn(move || {
                let mut previous = initial_size;
                while running.load(Ordering::Acquire) {
                    thread::sleep(RESIZE_POLL_INTERVAL);
                    let Ok((columns, rows)) = terminal_size() else {
                        continue;
                    };
                    if columns == 0 || rows == 0 {
                        continue;
                    }
                    let size = PtySize {
                        rows,
                        cols: columns,
                        pixel_width: 0,
                        pixel_height: 0,
                    };
                    if size == previous {
                        continue;
                    }

                    let Ok(master) = master.lock() else {
                        break;
                    };
                    let Some(master) = master.as_ref() else {
                        break;
                    };
                    if master.resize(size).is_ok() {
                        previous = size;
                        let _ = sender.send(size);
                    }
                }
            })
            .map_err(|error| format!("cannot start terminal resize forwarding: {error}"))?;

        Ok(Self {
            receiver,
            handle: Some(handle),
        })
    }

    fn stop(mut self) -> Result<(), String> {
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| "terminal resize forwarding thread panicked".to_owned())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod input_filter_tests {
    use super::{RENDER_TOGGLE_SEQUENCE, SessionInputAction, SessionInputFilter};

    fn filter_chunks(chunks: &[&[u8]]) -> Vec<SessionInputAction> {
        let mut filter = SessionInputFilter::default();
        let mut actions = Vec::new();
        for chunk in chunks {
            filter.push(chunk, &mut actions);
        }
        filter.finish(&mut actions);
        actions
    }

    fn flatten(actions: &[SessionInputAction]) -> (Vec<u8>, usize) {
        let mut forwarded = Vec::new();
        let mut toggles = 0_usize;
        for action in actions {
            match action {
                SessionInputAction::Forward(bytes) => forwarded.extend_from_slice(bytes),
                SessionInputAction::ToggleRendering => toggles += 1,
            }
        }
        (forwarded, toggles)
    }

    #[test]
    fn exact_control_sequence_is_consumed_in_input_order() {
        let mut input = b"before".to_vec();
        input.extend_from_slice(RENDER_TOGGLE_SEQUENCE);
        input.extend_from_slice(b"after");
        let actions = filter_chunks(&[input.as_slice()]);
        assert_eq!(
            actions,
            vec![
                SessionInputAction::Forward(b"before".to_vec()),
                SessionInputAction::ToggleRendering,
                SessionInputAction::Forward(b"after".to_vec()),
            ]
        );
    }

    #[test]
    fn every_control_sequence_split_is_recognized() {
        for split in 0..=RENDER_TOGGLE_SEQUENCE.len() {
            let actions = filter_chunks(&[
                &RENDER_TOGGLE_SEQUENCE[..split],
                &RENDER_TOGGLE_SEQUENCE[split..],
            ]);
            assert_eq!(
                actions,
                vec![SessionInputAction::ToggleRendering],
                "split={split}"
            );
        }
    }

    #[test]
    fn ordinary_escape_is_forwarded_without_waiting_for_more_input() {
        let mut filter = SessionInputFilter::default();
        let mut actions = Vec::new();
        filter.push(b"\x1b", &mut actions);
        assert_eq!(actions, vec![SessionInputAction::Forward(vec![0x1b])]);
        assert!(filter.pending.is_empty());
    }

    #[test]
    fn every_mismatching_prefix_is_forwarded_byte_exact() {
        for mismatch_at in 0..RENDER_TOGGLE_SEQUENCE.len() {
            let mut input = RENDER_TOGGLE_SEQUENCE[..mismatch_at].to_vec();
            let expected = RENDER_TOGGLE_SEQUENCE[mismatch_at];
            input.push(expected ^ 0x01);
            input.extend_from_slice(b" ordinary input");
            let actions = filter_chunks(&[input.as_slice()]);
            assert_eq!(flatten(&actions), (input, 0), "mismatch_at={mismatch_at}");
        }
    }

    #[test]
    fn every_unfinished_prefix_is_forwarded_at_end_of_input() {
        for length in 0..RENDER_TOGGLE_SEQUENCE.len() {
            let input = &RENDER_TOGGLE_SEQUENCE[..length];
            let actions = filter_chunks(&[input]);
            assert_eq!(flatten(&actions), (input.to_vec(), 0), "length={length}");
        }
    }

    #[test]
    fn repeated_sequences_report_each_toggle() {
        let mut input = RENDER_TOGGLE_SEQUENCE.to_vec();
        input.extend_from_slice(RENDER_TOGGLE_SEQUENCE);
        let actions = filter_chunks(&[input.as_slice()]);
        assert_eq!(
            actions,
            vec![
                SessionInputAction::ToggleRendering,
                SessionInputAction::ToggleRendering,
            ]
        );
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::{NativeTerminalSession, PtySize};
    use crate::command::ChildCommand;
    use std::ffi::OsString;
    use std::io::{self, Read};

    fn read_pty_to_end(reader: &mut dyn Read) -> Vec<u8> {
        let mut output = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => output.extend_from_slice(&buffer[..count]),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.raw_os_error() == Some(libc::EIO) => break,
                Err(error) => panic!("read PTY output: {error}"),
            }
        }
        output
    }

    #[test]
    fn real_unix_pty_resize_reaches_the_child() {
        let command = ChildCommand::from_argv(
            vec![
                OsString::from("/bin/sh"),
                OsString::from("-c"),
                OsString::from("sleep 1; stty size"),
            ],
            "missing child command",
        )
        .expect("command");
        let mut session = NativeTerminalSession::spawn(
            &command,
            PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            },
        )
        .expect("spawn real PTY child");
        session
            .resize(PtySize {
                rows: 40,
                cols: 100,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("resize real PTY");

        let output = read_pty_to_end(session.output_reader());
        let status = session.wait().expect("wait for PTY child");
        assert!(status.success(), "{status}");
        assert!(
            String::from_utf8_lossy(&output).contains("40 100"),
            "{}",
            String::from_utf8_lossy(&output)
        );
    }
}
