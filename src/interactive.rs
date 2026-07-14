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

    options.color = options.color || parent.output_is_terminal();
    options.columns = Some(parent.initial_size().cols);
    let mut pipeline = PipelineFactory::new(&config).build(options);
    let output_result = {
        let stdout = io::stdout();
        let mut display = stdout.lock();
        PipelinePump::interactive().run_with_updates(
            session.output_reader(),
            &mut display,
            &mut pipeline,
            |pipeline| {
                if let Some(size) = control.latest_resize() {
                    pipeline.set_columns(size.cols);
                }
            },
        )
    };
    let control_result = control.stop();

    if let Err(output_error) = output_result {
        let _ = session.kill();
        let _ = session.wait();
        return match control_result {
            Ok(()) => Err(format!("cannot process child PTY output: {output_error}")),
            Err(control_error) => Err(format!(
                "cannot process child PTY output: {output_error}; {control_error}"
            )),
        };
    }
    control_result?;

    let status = session.wait()?;
    Ok(normalize_exit_code(&status))
}
