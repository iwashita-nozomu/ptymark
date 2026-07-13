use crate::cache::{ArtifactCache, MemoryCache, NoopCache};
use crate::config::{Config, RenderMode};
use crate::detector::{FencedDetector, PassthroughDetector, SemanticDetector};
use crate::pipeline::DisplayPipeline;
use crate::render::{RenderContext, RenderService, Renderer, SourceRenderer};
use crate::routing::RoutedRenderer;
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub const HELP: &str = "\

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

pub fn is_top_level_help(arguments: &[OsString]) -> bool {
    matches!(
        arguments.first().and_then(|value| value.to_str()),
        Some("-h" | "--help")
    )
}

pub fn run_from(mut arguments: Vec<OsString>) -> Option<Result<i32, String>> {
    let mut config_path = None;

    while matches!(
        arguments.first().and_then(|value| value.to_str()),
        Some("--config")
    ) {
        arguments.remove(0);
        let path = match arguments.first().cloned() {
            Some(path) => path,
            None => return Some(Err("missing value after `--config`".to_owned())),
        };
        arguments.remove(0);
        if config_path.replace(PathBuf::from(path)).is_some() {
            return Some(Err("`--config` may be specified only once".to_owned()));
        }
    }

    if !matches!(
        arguments.first().and_then(|value| value.to_str()),
        Some("run")
    ) {
        return None;
    }
    arguments.remove(0);
    Some(run_filtered(arguments, config_path))
}

fn run_filtered(
    arguments: Vec<OsString>,
    mut config_path: Option<PathBuf>,
) -> Result<i32, String> {
    let mut source = false;
    let mut strict = false;
    let mut no_cache = false;
    let mut color = false;
    let mut columns = None;
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
            "--source" => source = true,
            "--strict" => strict = true,
            "--no-cache" => no_cache = true,
            "--color" => color = true,
            "--columns" => columns = Some(next_columns(&mut iterator)?),
            "--config" => {
                let path = PathBuf::from(next_value(&mut iterator, "--config")?);
                if config_path.replace(path).is_some() {
                    return Err("`--config` may be specified only once".to_owned());
                }
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

    let program = command
        .first()
        .cloned()
        .ok_or_else(|| "missing child command after `run --`".to_owned())?;
    let config = Config::load(config_path.as_deref()).map_err(|error| error.to_string())?;
    let mut pipeline = build_pipeline(&config, source, strict, no_cache, color, columns);

    let mut child = Command::new(&program)
        .args(&command[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| {
            format!(
                "cannot execute `{}`: {error}",
                program.to_string_lossy()
            )
        })?;
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
    let status = child.wait().map_err(|error| {
        format!(
            "cannot wait for `{}`: {error}",
            program.to_string_lossy()
        )
    })?;
    Ok(status.code().unwrap_or(1))
}

pub(crate) fn build_pipeline(
    config: &Config,
    source: bool,
    strict: bool,
    no_cache: bool,
    color: bool,
    columns: Option<u16>,
) -> DisplayPipeline {
    let detector: Box<dyn SemanticDetector> =
        if config.detection.math || config.detection.mermaid {
            Box::new(FencedDetector::new(&config.detection))
        } else {
            Box::new(PassthroughDetector)
        };
    let source_mode = source || config.rendering.mode == RenderMode::Source;
    let renderer: Box<dyn Renderer> = if source_mode {
        Box::new(SourceRenderer)
    } else {
        Box::new(RoutedRenderer::configured(&config.engines))
    };
    let cache: Box<dyn ArtifactCache> =
        if source_mode || no_cache || !config.cache.enabled {
            Box::new(NoopCache::default())
        } else {
            Box::new(MemoryCache::new(
                config.cache.max_entries,
                config.cache.max_bytes,
            ))
        };
    let service = RenderService::new(renderer, cache);
    let context = RenderContext {
        columns: columns.unwrap_or(config.rendering.columns),
        color,
        theme_fingerprint: 0,
    };
    DisplayPipeline::new(
        detector,
        service,
        context,
        strict || config.rendering.strict,
    )
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
