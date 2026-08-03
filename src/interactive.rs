use crate::command::ChildCommand;
use crate::config::Config;
use crate::native_session::{
    NativeTerminalSession, ParentTerminal, SessionControl, normalize_exit_code,
};
use crate::runtime::{PipelineFactory, PipelineOptions};
use crate::stream::PipelinePump;
use std::env;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::thread::JoinHandle;

pub(crate) fn run(
    command: Vec<OsString>,
    config_path: Option<PathBuf>,
    profile: Option<String>,
    mut options: PipelineOptions,
    allow_nested: bool,
) -> Result<i32, String> {
    let command = ChildCommand::from_argv(command, "missing command after `shell --`")?;
    if active_session() && !allow_nested {
        return Err(
            "already running inside Ptymark.\nExit the current session first, or pass `--allow-nested` for development and debugging."
                .to_owned(),
        );
    }

    let config = Config::load_profile(config_path.as_deref(), profile.as_deref())
        .map_err(|error| error.to_string())?;
    let parent = ParentTerminal::detect(options.columns.unwrap_or(config.rendering.columns));
    let mut session = NativeTerminalSession::spawn(&command, parent.initial_size())?;
    let _raw_mode = parent.enter_raw_mode()?;
    let control = SessionControl::start(&mut session, parent)?;
    let output_killer = session.kill_handle();
    let waiter = match session.start_exit_waiter() {
        Ok(waiter) => waiter,
        Err(waiter_error) => {
            let kill_error = session.kill().err();
            let control_error = control.stop().err();
            return finish_session(None, control_error, Err(waiter_error), kill_error);
        }
    };

    #[cfg(windows)]
    if parent.needs_cursor_position_fallback()
        && let Err(error) = control.input_responder().send_cursor_position()
    {
        let kill_error = session.kill().err();
        let control_error = control.stop().err();
        return finish_session(
            Some(format!(
                "cannot answer ConPTY cursor position request: {error}"
            )),
            control_error,
            join_exit_waiter(waiter),
            kill_error,
        );
    }

    options.color = options.color || parent.output_is_terminal();
    options.columns = Some(parent.initial_size().cols);
    let mut pipeline = PipelineFactory::new(&config).build(options);
    pipeline.set_terminal_line_endings(true);
    let output_result = {
        let stdout = io::stdout();
        let mut display = stdout.lock();
        PipelinePump::interactive()
            .run_bounded_with_updates(
                session.output_reader(),
                &mut display,
                &mut pipeline,
                |pipeline| {
                    if let Some(size) = control.latest_resize() {
                        pipeline.set_columns(size.cols);
                    }
                },
                || {
                    let _ = output_killer.kill();
                },
            )
            .map_err(|error| format!("cannot process child PTY output: {error}"))
    };
    let kill_error = if output_result.is_err() {
        session.kill().err()
    } else {
        None
    };
    let control_error = control.stop().err();
    let status_result = join_exit_waiter(waiter);

    finish_session(
        output_result.err(),
        control_error,
        status_result,
        kill_error,
    )
}

fn active_session() -> bool {
    matches!(env::var("PTYMARK_ACTIVE").as_deref(), Ok("1"))
}

fn join_exit_waiter(
    waiter: JoinHandle<Result<portable_pty::ExitStatus, String>>,
) -> Result<portable_pty::ExitStatus, String> {
    waiter
        .join()
        .map_err(|_| "child process exit waiter panicked".to_owned())?
}

fn finish_session(
    output_error: Option<String>,
    control_error: Option<String>,
    status_result: Result<portable_pty::ExitStatus, String>,
    kill_error: Option<String>,
) -> Result<i32, String> {
    let mut errors = Vec::new();
    if let Some(error) = output_error {
        errors.push(error);
    }
    if let Some(error) = kill_error {
        errors.push(error);
    }
    if let Some(error) = control_error {
        errors.push(error);
    }

    let status = match status_result {
        Ok(status) => Some(status),
        Err(error) => {
            errors.push(error);
            None
        }
    };

    if errors.is_empty() {
        Ok(normalize_exit_code(
            status
                .as_ref()
                .expect("successful session has a child exit status"),
        ))
    } else {
        Err(errors.join("; "))
    }
}
