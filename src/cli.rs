use crate::cache::{MemoryCache, NoopCache};
use crate::config::{Config, RenderMode};
use crate::detector::{FencedDetector, PassthroughDetector, SemanticDetector};
use crate::pipeline::DisplayPipeline;
use crate::render::{PreviewRenderer, RenderContext, RenderService, Renderer, SourceRenderer};
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{self, Command};

pub const HELP: &str = "\
ptymark — pre-display renderer for terminal streams

USAGE:
    ptymark [--config PATH] preview [OPTIONS] [FILE|-]
    ptymark [--config PATH] config check
    ptymark [--config PATH] config show
    ptymark [--config PATH] -- COMMAND [ARG...]

PREVIEW OPTIONS:
    --source              keep complete semantic blocks as source
    --strict              fail instead of restoring source after renderer errors
    --no-cache            disable the in-memory render cache
    --color               allow ANSI color in the built-in preview renderer
    --columns N           renderer width hint
    --config PATH         use an explicit ptymark TOML file
    -h, --help            print this help

GLOBAL OPTIONS:
    --config PATH         validate and use an explicit ptymark TOML file
    -h, --help            print this help
    -V, --version         print the version

EXAMPLES:
    printf '$$\\nE = mc^2\\n$$\\n' | ptymark preview
    ptymark preview --source README.md
    ptymark config check --config examples/ptymark.toml
    ptymark -- codex
";

pub fn main_entry() -> ! {
    match run_from(env::args_os().skip(1).collect()) {
        Ok(code) => process::exit(code),
        Err(message) => {
            eprintln!("ptymark: {message}");
            process::exit(2);
        }
    }
}

pub fn run_from(mut arguments: Vec<OsString>) -> Result<i32, String> {
    if matches!(
        arguments.first().and_then(|value| value.to_str()),
        Some("-h" | "--help")
    ) {
        print!("{HELP}");
        return Ok(0);
    }
    if matches!(
        arguments.first().and_then(|value| value.to_str()),
        Some("-V" | "--version")
    ) {
        println!("ptymark {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }

    let mut config_path = None;
    parse_leading_config(&mut arguments, &mut config_path)?;

    let command = arguments
        .first()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "missing command; use `preview`, `config`, or `-- COMMAND`".to_owned())?
        .to_owned();
    arguments.remove(0);

    match command.as_str() {
        "preview" => run_preview(arguments, config_path),
        "config" => run_config(arguments, config_path),
        "--" => run_command(arguments, config_path),
        option if option.starts_with('-') => Err(format!("unknown option `{option}`")),
        _ => Err("child commands must follow `--`; example: `ptymark -- zsh -l`".to_owned()),
    }
}

fn parse_leading_config(
    arguments: &mut Vec<OsString>,
    config_path: &mut Option<PathBuf>,
) -> Result<(), String> {
    while matches!(
        arguments.first().and_then(|value| value.to_str()),
        Some("--config")
    ) {
        arguments.remove(0);
        let path = arguments
            .first()
            .cloned()
            .ok_or_else(|| "missing value after `--config`".to_owned())?;
        arguments.remove(0);
        set_config(config_path, PathBuf::from(path))?;
    }
    Ok(())
}

fn set_config(target: &mut Option<PathBuf>, path: PathBuf) -> Result<(), String> {
    if target.replace(path).is_some() {
        return Err("`--config` may be specified only once".to_owned());
    }
    Ok(())
}

fn next_path(
    iterator: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<PathBuf, String> {
    iterator
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing value after `{option}`"))
}

fn next_columns(iterator: &mut impl Iterator<Item = OsString>) -> Result<u16, String> {
    let value = iterator
        .next()
        .ok_or_else(|| "missing value after `--columns`".to_owned())?
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

fn run_preview(arguments: Vec<OsString>, mut config_path: Option<PathBuf>) -> Result<i32, String> {
    let mut source = false;
    let mut strict = false;
    let mut no_cache = false;
    let mut color = false;
    let mut columns = None;
    let mut input_path = None;
    let mut iterator = arguments.into_iter();

    while let Some(argument) = iterator.next() {
        let text = argument
            .to_str()
            .ok_or_else(|| "preview options must be valid UTF-8".to_owned())?;
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
            "--config" => set_config(&mut config_path, next_path(&mut iterator, "--config")?)?,
            "-" => {
                if input_path.replace(PathBuf::from("-")).is_some() {
                    return Err("preview accepts at most one input".to_owned());
                }
            }
            option if option.starts_with('-') => {
                return Err(format!("unknown preview option `{option}`"));
            }
            _ => {
                if input_path.replace(PathBuf::from(argument)).is_some() {
                    return Err("preview accepts at most one input".to_owned());
                }
            }
        }
    }

    let config = Config::load(config_path.as_deref()).map_err(|error| error.to_string())?;
    let detector: Box<dyn SemanticDetector> = if config.detection.math || config.detection.mermaid {
        Box::new(FencedDetector::new(&config.detection))
    } else {
        Box::new(PassthroughDetector)
    };

    let source_mode = source || config.rendering.mode == RenderMode::Source;
    let renderer: Box<dyn Renderer> = if source_mode {
        Box::new(SourceRenderer)
    } else {
        Box::new(PreviewRenderer)
    };
    let cache: Box<dyn crate::cache::ArtifactCache> =
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
    let mut pipeline = DisplayPipeline::new(
        detector,
        service,
        context,
        strict || config.rendering.strict,
    );

    let mut input: Box<dyn Read> = match input_path {
        Some(path) if path != *"-" => Box::new(
            File::open(&path)
                .map_err(|error| format!("cannot open `{}`: {error}", path.display()))?,
        ),
        _ => Box::new(io::stdin()),
    };
    let stdout = io::stdout();
    let mut display = stdout.lock();
    let mut buffer = [0_u8; 8192];

    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|error| format!("cannot read preview input: {error}"))?;
        if count == 0 {
            break;
        }
        pipeline
            .feed(&buffer[..count], &mut display)
            .map_err(|error| error.to_string())?;
    }
    pipeline
        .finish(&mut display)
        .map_err(|error| error.to_string())?;
    display.flush().map_err(|error| error.to_string())?;
    Ok(0)
}

fn run_config(
    mut arguments: Vec<OsString>,
    mut config_path: Option<PathBuf>,
) -> Result<i32, String> {
    let action = arguments
        .first()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "missing config action; use `check` or `show`".to_owned())?
        .to_owned();
    arguments.remove(0);

    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        match argument.to_str() {
            Some("--config") => {
                set_config(&mut config_path, next_path(&mut iterator, "--config")?)?
            }
            Some("-h" | "--help") => {
                print!("{HELP}");
                return Ok(0);
            }
            Some(option) => return Err(format!("unknown config option `{option}`")),
            None => return Err("config options must be valid UTF-8".to_owned()),
        }
    }

    let config = Config::load(config_path.as_deref()).map_err(|error| error.to_string())?;
    match action.as_str() {
        "check" => {
            println!(
                "configuration ok: schema={} renderer={:?}",
                config.schema_version, config.rendering.mode
            );
            Ok(0)
        }
        "show" => {
            print!("{}", config.to_toml().map_err(|error| error.to_string())?);
            Ok(0)
        }
        _ => Err(format!(
            "unknown config action `{action}`; use `check` or `show`"
        )),
    }
}

fn run_command(arguments: Vec<OsString>, config_path: Option<PathBuf>) -> Result<i32, String> {
    let program = arguments
        .first()
        .cloned()
        .ok_or_else(|| "missing command after `--`".to_owned())?;
    Config::load(config_path.as_deref()).map_err(|error| error.to_string())?;

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = Command::new(&program).args(&arguments[1..]).exec();
        Err(format!(
            "cannot execute `{}`: {error}",
            program.to_string_lossy()
        ))
    }

    #[cfg(not(unix))]
    {
        let status = Command::new(&program)
            .args(&arguments[1..])
            .status()
            .map_err(|error| format!("cannot execute `{}`: {error}", program.to_string_lossy()))?;
        Ok(status.code().unwrap_or(1))
    }
}
