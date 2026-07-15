use crate::cli_args::apply_render_option;
use crate::command::ChildCommand;
use crate::config::Config;
use crate::runtime::{PipelineFactory, PipelineOptions};
use crate::stream::PipelinePump;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub(crate) const HELP: &str = "\
FILTERED COMMAND RUNNER:
    ptymark [--config PATH] run [OPTIONS] -- COMMAND [ARG...]

    Process one non-interactive child's stdout through the pre-display pipeline.
    Child stdin and stderr are inherited directly; stdout is a pipe, not a PTY.

RUN OPTIONS:
    --source              detect complete blocks but display their exact source
    --safe                bypass semantic detection and rendering for this session
    --private             disable cache and persistence-capable diagnostics for this session
    --strict              fail instead of restoring source after renderer errors
    --no-cache            disable the in-memory render cache
    --color               allow ANSI color in terminal renderers
    --columns N           renderer width hint
    --config PATH         use an explicit ptymark TOML file
    -h, --help            print this help

EXAMPLE:
    ptymark run -- command-that-prints-markdown
";

pub(crate) fn run(
    arguments: Vec<OsString>,
    mut config_path: Option<PathBuf>,
) -> Result<i32, String> {
    let mut options = PipelineOptions::default();
    let mut command = Vec::new();
    let mut iterator = arguments.into_iter();

    while let Some(argument) = iterator.next() {
        let text = argument.to_str().ok_or_else(|| {
            "run options must be valid UTF-8; place the child command after `--`".to_owned()
        })?;
        if apply_render_option(text, &mut iterator, &mut options, &mut config_path)? {
            continue;
        }
        match text {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(0);
            }
            "--" => {
                command.extend(iterator);
                break;
            }
            option => {
                return Err(format!(
                    "unknown run option `{option}`; child commands must follow `--`"
                ));
            }
        }
    }

    let command = ChildCommand::from_argv(command, "missing child command after `run --`")?;
    let config = Config::load(config_path.as_deref()).map_err(|error| error.to_string())?;
    let mut pipeline = PipelineFactory::new(&config).build(options);

    let mut child = Command::new(command.program())
        .args(command.arguments())
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("cannot execute `{}`: {error}", command.display_name()))?;
    let mut child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| "child stdout pipe is unavailable".to_owned())?;
    let stdout = io::stdout();
    let mut display = stdout.lock();

    if let Err(error) = PipelinePump::standard().run_bounded_with_updates(
        &mut child_stdout,
        &mut display,
        &mut pipeline,
        |_| {},
        || terminate_child(&mut child),
    ) {
        terminate_child(&mut child);
        return Err(format!("cannot process child stdout: {error}"));
    }

    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for `{}`: {error}", command.display_name()))?;
    Ok(status.code().unwrap_or(1))
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
