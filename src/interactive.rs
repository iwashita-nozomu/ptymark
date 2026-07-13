use crate::command::ChildCommand;
use crate::config::Config;
use crate::pipeline::DisplayPipeline;
use crate::runtime::{PipelineFactory, PipelineOptions};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, size as terminal_size};
use portable_pty::{
    Child as PtyChild, CommandBuilder, MasterPty, PtyPair, PtySize, native_pty_system,
};
use std::env;
use std::ffi::OsString;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const DEFAULT_ROWS: u16 = 24;
const RESIZE_POLL_INTERVAL: Duration = Duration::from_millis(80);

pub(crate) fn run(
    command: Vec<OsString>,
    config_path: Option<PathBuf>,
) -> Result<i32, String> {
    let command = ChildCommand::from_argv(command, "missing command after `--`")?;
    let config = Config::load(config_path.as_deref()).map_err(|error| error.to_string())?;
    let stdin_is_terminal = io::stdin().is_terminal();
    let stdout_is_terminal = io::stdout().is_terminal();
    let initial_size = initial_pty_size(config.rendering.columns, stdout_is_terminal);
    let mut session = NativePtySession::spawn(&command, initial_size)?;
    let _raw_mode = RawModeGuard::acquire(stdin_is_terminal && stdout_is_terminal)?;

    let running = Arc::new(AtomicBool::new(true));
    let writer = session.take_writer()?;
    let _input_pump = spawn_input_pump(writer, Arc::clone(&running))?;
    let resize_monitor = ResizeMonitor::spawn(
        session.resize_handle(),
        Arc::clone(&running),
        initial_size,
        stdout_is_terminal,
    )?;

    let mut pipeline = PipelineFactory::new(&config).build(PipelineOptions {
        color: stdout_is_terminal,
        columns: Some(initial_size.cols),
        ..PipelineOptions::default()
    });
    let output_result = pump_output(
        session.reader_mut(),
        &mut pipeline,
        resize_monitor.receiver(),
    );

    running.store(false, Ordering::Release);
    let resize_result = resize_monitor.stop();

    if let Err(error) = output_result {
        let _ = session.kill();
        let _ = session.wait();
        resize_result?;
        return Err(error);
    }
    resize_result?;

    let status = session.wait()?;
    Ok(normalize_exit_code(&status))
}

fn pump_output(
    reader: &mut dyn Read,
    pipeline: &mut DisplayPipeline,
    resize_events: &Receiver<PtySize>,
) -> Result<(), String> {
    let stdout = io::stdout();
    let mut display = stdout.lock();
    let mut buffer = [0_u8; 8192];

    loop {
        apply_resize_events(pipeline, resize_events);
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                // A resize may have arrived while the blocking read was in progress.
                // Drain it before rendering the newly read bytes so the next complete
                // semantic block uses the latest width.
                apply_resize_events(pipeline, resize_events);
                pipeline
                    .feed(&buffer[..count], &mut display)
                    .map_err(|error| error.to_string())?;
                display.flush().map_err(|error| error.to_string())?;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if is_pty_eof(&error) => break,
            Err(error) => return Err(format!("cannot read child PTY output: {error}")),
        }
    }

    apply_resize_events(pipeline, resize_events);
    pipeline
        .finish(&mut display)
        .map_err(|error| error.to_string())?;
    display.flush().map_err(|error| error.to_string())
}

fn apply_resize_events(pipeline: &mut DisplayPipeline, resize_events: &Receiver<PtySize>) {
    while let Ok(size) = resize_events.try_recv() {
        pipeline.set_columns(size.cols);
    }
}

fn spawn_input_pump(
    mut writer: Box<dyn Write + Send>,
    running: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, String> {
    thread::Builder::new()
        .name("ptymark-stdin".to_owned())
        .spawn(move || {
            let stdin = io::stdin();
            let mut input = stdin.lock();
            let mut buffer = [0_u8; 8192];

            while running.load(Ordering::Acquire) {
                let count = match input.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => count,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                };
                if writer.write_all(&buffer[..count]).is_err() || writer.flush().is_err() {
                    break;
                }
            }
        })
        .map_err(|error| format!("cannot start terminal input forwarding: {error}"))
}

fn initial_pty_size(fallback_columns: u16, terminal_attached: bool) -> PtySize {
    if terminal_attached {
        if let Ok((columns, rows)) = terminal_size() {
            if columns > 0 && rows > 0 {
                return PtySize {
                    rows,
                    cols: columns,
                    pixel_width: 0,
                    pixel_height: 0,
                };
            }
        }
    }

    PtySize {
        rows: environment_dimension("LINES").unwrap_or(DEFAULT_ROWS),
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

fn is_pty_eof(error: &io::Error) -> bool {
    if matches!(
        error.kind(),
        io::ErrorKind::UnexpectedEof | io::ErrorKind::BrokenPipe
    ) {
        return true;
    }

    #[cfg(unix)]
    if error.raw_os_error() == Some(libc::EIO) {
        return true;
    }

    false
}

fn normalize_exit_code(status: &portable_pty::ExitStatus) -> i32 {
    i32::try_from(status.exit_code()).unwrap_or(1)
}

struct RawModeGuard {
    enabled: bool,
}

impl RawModeGuard {
    fn acquire(enabled: bool) -> Result<Self, String> {
        if enabled {
            enable_raw_mode().map_err(|error| format!("cannot enable terminal raw mode: {error}"))?;
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

type SharedMaster = Arc<Mutex<Box<dyn MasterPty + Send>>>;

struct NativePtySession {
    master: SharedMaster,
    reader: Box<dyn Read + Send>,
    writer: Option<Box<dyn Write + Send>>,
    child: Box<dyn PtyChild + Send + Sync>,
}

impl NativePtySession {
    fn spawn(command: &ChildCommand, size: PtySize) -> Result<Self, String> {
        let system = native_pty_system();
        let PtyPair { master, slave } = system
            .openpty(size)
            .map_err(|error| format!("cannot allocate native PTY: {error}"))?;

        let mut builder = CommandBuilder::new(command.program());
        builder.args(command.arguments());
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

        Ok(Self {
            master: Arc::new(Mutex::new(master)),
            reader,
            writer: Some(writer),
            child,
        })
    }

    fn reader_mut(&mut self) -> &mut dyn Read {
        self.reader.as_mut()
    }

    fn take_writer(&mut self) -> Result<Box<dyn Write + Send>, String> {
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
            .resize(size)
            .map_err(|error| format!("cannot resize child PTY: {error}"))
    }

    fn kill(&mut self) -> Result<(), String> {
        self.child
            .kill()
            .map_err(|error| format!("cannot terminate child process: {error}"))
    }

    fn wait(&mut self) -> Result<portable_pty::ExitStatus, String> {
        self.child
            .wait()
            .map_err(|error| format!("cannot wait for child process: {error}"))
    }
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

    fn receiver(&self) -> &Receiver<PtySize> {
        &self.receiver
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

#[cfg(all(test, unix))]
mod tests {
    use super::{NativePtySession, PtySize};
    use crate::command::ChildCommand;
    use std::ffi::OsString;
    use std::io::Read;

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
        let mut session = NativePtySession::spawn(
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

        let mut output = Vec::new();
        session
            .reader_mut()
            .read_to_end(&mut output)
            .expect("read PTY output");
        let status = session.wait().expect("wait for PTY child");
        assert!(status.success(), "{status}");
        assert!(
            String::from_utf8_lossy(&output).contains("40 100"),
            "{}",
            String::from_utf8_lossy(&output)
        );
    }
}
