use crate::cli_args::apply_render_option;
use crate::command::ChildCommand;
use crate::config::Config;
use crate::native_session::{
    NativeTerminalSession, ParentTerminal, SessionControl, normalize_exit_code,
};
use crate::runtime::{PipelineFactory, PipelineOptions};
use crate::stream::PipelinePump;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::thread::JoinHandle;

pub(crate) fn run(
    arguments: Vec<OsString>,
    mut config_path: Option<PathBuf>,
) -> Result<i32, String> {
    let mut options = PipelineOptions::default();
    let mut command = Vec::new();
    let mut iterator = arguments.into_iter();

    while let Some(argument) = iterator.next() {
        let text = argument.to_str().ok_or_else(|| {
            "interactive options must be valid UTF-8; place the child command after `--`".to_owned()
        })?;
        if apply_render_option(text, &mut iterator, &mut options, &mut config_path)? {
            continue;
        }
        match text {
            "-h" | "--help" => {
                print!("{}", crate::cli::HELP);
                return Ok(0);
            }
            "--" => {
                command.extend(iterator);
                break;
            }
            option => {
                return Err(format!(
                    "unknown interactive option `{option}`; child commands must follow `--`"
                ));
            }
        }
    }

    let command = ChildCommand::from_argv(command, "missing command after `--`")?;
    let config = Config::load(config_path.as_deref()).map_err(|error| error.to_string())?;
    let parent = ParentTerminal::detect(options.columns.unwrap_or(config.rendering.columns));
    let mut session = NativeTerminalSession::spawn(&command, parent.initial_size())?;
    let _raw_mode = parent.enter_raw_mode()?;
    let control = SessionControl::start(&mut session, parent)?;
    let waiter = match session.start_exit_waiter() {
        Ok(waiter) => waiter,
        Err(waiter_error) => {
            let kill_error = session.kill().err();
            let control_error = control.stop().err();
            return finish_session(None, control_error, Err(waiter_error), kill_error);
        }
    };

    options.color = options.color || parent.output_is_terminal();
    options.columns = Some(parent.initial_size().cols);
    let mut pipeline = PipelineFactory::new(&config).build(options);
    let output_result = {
        let stdout = io::stdout();
        let mut display = stdout.lock();
        PipelinePump::interactive()
            .run_with_updates(
                session.output_reader(),
                &mut display,
                &mut pipeline,
                |pipeline| {
                    if let Some(size) = control.latest_resize() {
                        pipeline.set_columns(size.cols);
                    }
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

    finish_session(output_result.err(), control_error, status_result, kill_error)
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
