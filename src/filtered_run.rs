use crate::command::ChildCommand;
use crate::config::Config;
use crate::runtime::{PipelineFactory, PipelineOptions};
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub(crate) const HELP: &str = "\
FILTERED COMMAND RUNNER:
    ptymark [--config PATH] run [OPTIONS] -- COMMAND [ARG...]

    Process one non-interactive child's stdout through the pre-display pipeline.
    Child stdin and stderr are inherited directly; stdout is a pipe, not a PTY.

RUN OPTIONS:
    --source              keep complete semantic blocks as source
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
        match text {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(0);
            }
            "--source" => options.source = true,
            "--strict" => options.strict = true,
            "--no-cache" => options.no_cache = true,
            "--color" => options.color = true,
            "--columns" => options.columns = Some(next_columns(&mut iterator)?),
            "--config" => {
                let path = PathBuf::from(next_value(&mut iterator, "--config")?);
                set_config(&mut config_path, path)?;
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
    let mut buffer = [0_u8; 8192];

    loop {
        let count = match child_stdout.read(&mut buffer) {
            Ok(count) => count,
            Err(error) => {
                terminate_child(&mut child);
                return Err(format!("cannot read child stdout: {error}"));
            }
        };
        if count == 0 {
            break;
        }
        if let Err(error) = pipeline.feed(&buffer[..count], &mut display) {
            terminate_child(&mut child);
            return Err(error.to_string());
        }
    }

    if let Err(error) = pipeline.finish(&mut display) {
        terminate_child(&mut child);
        return Err(error.to_string());
    }
    display.flush().map_err(|error| error.to_string())?;
    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for `{}`: {error}", command.display_name()))?;
    Ok(status.code().unwrap_or(1))
}

fn set_config(target: &mut Option<PathBuf>, path: PathBuf) -> Result<(), String> {
    if target.replace(path).is_some() {
        return Err("`--config` may be specified only once".to_owned());
    }
    Ok(())
}

fn next_value(
    iterator: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<OsString, String> {
    iterator
        .next()
        .ok_or_else(|| format!("missing value after `{option}`"))
}

fn next_columns(iterator: &mut impl Iterator<Item = OsString>) -> Result<u16, String> {
    let value = next_value(iterator, "--columns")?
        .into_string()
        .map_err(|_| "`--columns` requires UTF-8 digits".to_owned())?;
    let columns = value
        .parse::<u16>()
        .map_err(|_| "`--columns` requires a positive integer".to_owned())?;
    if columns == 0 {
        return Err("`--columns` must be greater than zero".to_owned());
    }
    Ok(columns)
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
