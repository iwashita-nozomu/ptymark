use ptymark::{
    BlockRenderer, CacheBackend, CachePolicy, ConfigEnvironment, ConfigManager, ConfigOrigin,
    ConfigRequest, ConfigTrust, CoordinatedRenderer, DetectionMode, DisplayInterceptor,
    FallbackPolicy, FencedDetector, FencedDetectorOptions, MemoryArtifactCache,
    PassthroughDetector, PreDisplayRenderer, RenderContext, SemanticDetector, SessionMode,
    SourceRenderer, TerminalOutputGate, stable_fingerprint,
};
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{self, Command};

const HELP: &str = "\
ptymark — pre-display semantic renderer for terminal streams

USAGE:
    ptymark -- COMMAND [ARG...]
    ptymark preview [OPTIONS] [FILE|-]
    ptymark demo [OPTIONS]
    ptymark config paths
    ptymark config check [CONFIG OPTIONS]
    ptymark config show [CONFIG OPTIONS] [--provenance]

COMMANDS:
    preview     render bounded Mermaid and block-math input before display
    demo        render a built-in sample through the same pre-display pipeline
    config      inspect and validate the immutable session configuration

PREVIEW OPTIONS:
    --source                 emit the original semantic source instead of a preview
    --strict                 fail instead of restoring source when rendering fails
    --color                  emit ANSI color in compatible renderers
    --max-buffer-bytes N     override the configured semantic buffer limit
    --terminal-width N       width hint for renderer backends
    --no-cache               disable the configured in-process render cache
    --config PATH            load an explicit configuration after the user config
    --profile NAME           select a named configuration profile
    --no-config              ignore user and environment configuration files
    -h, --help               print this help

CONFIG OPTIONS:
    --config PATH            load an explicit configuration after the user config
    --profile NAME           select a named configuration profile
    --no-config              use built-in profiles only

GLOBAL OPTIONS:
    -h, --help               print this help
    -V, --version            print the version

EXAMPLES:
    ptymark -- zsh -l
    printf '$$\\nE = mc^2\\n$$\\n' | ptymark preview
    ptymark preview --profile private --no-cache
    ptymark config check --config ./ptymark.toml
    ptymark config show --profile interactive --provenance
";

const DEMO: &[u8] = br#"ordinary output remains byte-for-byte passthrough

```mermaid
flowchart LR
    PTY --> Detector
    Detector --> Renderer
    Renderer --> Display
```

$$
E = mc^2
$$
"#;

#[derive(Clone, Debug, Default)]
struct ConfigOptions {
    explicit_path: Option<PathBuf>,
    profile: Option<String>,
    no_config: bool,
}

impl ConfigOptions {
    fn request(&self) -> ConfigRequest {
        ConfigRequest {
            explicit_path: self.explicit_path.clone(),
            profile: self.profile.clone(),
            no_config: self.no_config,
            ..ConfigRequest::default()
        }
    }
}

#[derive(Debug)]
struct PreviewOptions {
    source_renderer: bool,
    strict: bool,
    color: bool,
    cache: Option<bool>,
    max_buffer_bytes: Option<usize>,
    terminal_width: Option<usize>,
    config: ConfigOptions,
    input: PreviewInput,
}

#[derive(Debug)]
enum PreviewInput {
    Stdin,
    File(PathBuf),
    Demo,
}

fn main() {
    match run() {
        Ok(code) => process::exit(code),
        Err(message) => {
            eprintln!("ptymark: {message}");
            process::exit(2);
        }
    }
}

fn run() -> Result<i32, String> {
    let mut arguments: Vec<OsString> = env::args_os().skip(1).collect();
    let first = arguments
        .first()
        .and_then(|argument| argument.to_str())
        .ok_or_else(|| {
            "missing command; use `ptymark -- COMMAND`, `ptymark preview`, or `ptymark config`"
                .to_owned()
        })?;

    match first {
        "-h" | "--help" => {
            print!("{HELP}");
            Ok(0)
        }
        "-V" | "--version" => {
            println!("ptymark {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        "preview" => {
            arguments.remove(0);
            if subcommand_help_requested(&arguments)? {
                print!("{HELP}");
                return Ok(0);
            }
            run_preview(parse_preview(arguments, PreviewInput::Stdin)?)
        }
        "demo" => {
            arguments.remove(0);
            if subcommand_help_requested(&arguments)? {
                print!("{HELP}");
                return Ok(0);
            }
            run_preview(parse_preview(arguments, PreviewInput::Demo)?)
        }
        "config" => {
            arguments.remove(0);
            run_config(arguments)
        }
        "--" => {
            arguments.remove(0);
            run_command(arguments)
        }
        option if option.starts_with('-') => Err(format!("unknown option `{option}`")),
        _ => Err("commands must be introduced with `--`; example: `ptymark -- zsh -l`".to_owned()),
    }
}

fn subcommand_help_requested(arguments: &[OsString]) -> Result<bool, String> {
    let help_count = arguments
        .iter()
        .filter(|argument| matches!(argument.to_str(), Some("-h" | "--help")))
        .count();

    if help_count == 0 {
        return Ok(false);
    }
    if arguments.len() == 1 {
        return Ok(true);
    }

    Err("`--help` cannot be combined with other subcommand options".to_owned())
}

fn parse_preview(
    arguments: Vec<OsString>,
    default_input: PreviewInput,
) -> Result<PreviewOptions, String> {
    let is_demo = matches!(default_input, PreviewInput::Demo);
    let mut options = PreviewOptions {
        source_renderer: false,
        strict: false,
        color: false,
        cache: None,
        max_buffer_bytes: None,
        terminal_width: None,
        config: ConfigOptions::default(),
        input: default_input,
    };
    let mut positional_seen = false;
    let mut iterator = arguments.into_iter();

    while let Some(argument) = iterator.next() {
        let text = argument
            .to_str()
            .ok_or_else(|| "preview options must be valid UTF-8".to_owned())?;
        match text {
            "--source" => options.source_renderer = true,
            "--strict" => options.strict = true,
            "--color" => options.color = true,
            "--no-cache" => options.cache = Some(false),
            "--no-config" => options.config.no_config = true,
            "--config" => {
                options.config.explicit_path = Some(next_path(&mut iterator, "--config")?);
            }
            "--profile" => {
                options.config.profile = Some(next_string(&mut iterator, "--profile")?);
            }
            "--max-buffer-bytes" => {
                options.max_buffer_bytes =
                    Some(next_positive_usize(&mut iterator, "--max-buffer-bytes")?);
            }
            "--terminal-width" => {
                options.terminal_width =
                    Some(next_positive_usize(&mut iterator, "--terminal-width")?);
            }
            "-" => {
                if is_demo {
                    return Err("`ptymark demo` does not accept an input file".to_owned());
                }
                if positional_seen {
                    return Err("preview accepts at most one input file".to_owned());
                }
                options.input = PreviewInput::Stdin;
                positional_seen = true;
            }
            option if option.starts_with('-') => {
                return Err(format!("unknown preview option `{option}`"));
            }
            _ => {
                if is_demo {
                    return Err("`ptymark demo` does not accept an input file".to_owned());
                }
                if positional_seen {
                    return Err("preview accepts at most one input file".to_owned());
                }
                options.input = PreviewInput::File(PathBuf::from(argument));
                positional_seen = true;
            }
        }
    }

    Ok(options)
}

fn next_string(
    iterator: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<String, String> {
    let value = iterator
        .next()
        .ok_or_else(|| format!("missing value after `{option}`"))?
        .into_string()
        .map_err(|_| format!("value for `{option}` must be valid UTF-8"))?;
    if value.is_empty() {
        return Err(format!("value for `{option}` cannot be empty"));
    }
    Ok(value)
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

fn next_positive_usize(
    iterator: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<usize, String> {
    let value = next_string(iterator, option)?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("value for `{option}` must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("value for `{option}` must be greater than zero"));
    }
    Ok(parsed)
}

fn run_preview(options: PreviewOptions) -> Result<i32, String> {
    let loaded = ConfigManager::default()
        .load_from_process(options.config.request())
        .map_err(|error| error.to_string())?;
    let session = &loaded.config;

    let source_renderer = options.source_renderer || session.mode == SessionMode::Source;
    let cache_enabled = options.cache.unwrap_or(
        session.cache.backend == CacheBackend::Memory && !session.cache.private,
    );
    let renderer: Box<dyn BlockRenderer> = if source_renderer {
        Box::new(SourceRenderer)
    } else if cache_enabled {
        Box::new(
            CoordinatedRenderer::preview(MemoryArtifactCache::new(CachePolicy::new(
                session.cache.max_entries,
                session.cache.max_bytes,
            )))
            .map_err(|error| error.to_string())?,
        )
    } else {
        Box::new(
            CoordinatedRenderer::preview(ptymark::NoopArtifactCache::default())
                .map_err(|error| error.to_string())?,
        )
    };

    if !matches!(session.cache.backend, CacheBackend::None | CacheBackend::Memory) && cache_enabled {
        return Err(format!(
            "cache backend `{:?}` is configured but not implemented in the preview runtime",
            session.cache.backend
        ));
    }

    let detection_enabled = session.mode != SessionMode::Bypass
        && session.detection.mode != DetectionMode::Off;
    let detector: Box<dyn SemanticDetector> = if detection_enabled {
        Box::new(FencedDetector::with_options(FencedDetectorOptions {
            max_buffer_bytes: options
                .max_buffer_bytes
                .unwrap_or(session.detection.max_buffer_bytes),
            max_line_bytes: session.detection.max_line_bytes,
            mermaid: session.detection.mermaid,
            block_math: session.detection.block_math,
        }))
    } else {
        Box::new(PassthroughDetector)
    };

    let effective = loaded.effective_toml().map_err(|error| error.to_string())?;
    let context = RenderContext {
        color: options.color,
        terminal_width: options.terminal_width,
        theme_fingerprint: 0,
        options_fingerprint: stable_fingerprint(effective.as_bytes()),
    };
    let pre_display = PreDisplayRenderer::new(detector, renderer)
        .with_context(context)
        .strict(options.strict || session.fallback == FallbackPolicy::Error);
    let mut interceptor = DisplayInterceptor::new(TerminalOutputGate::default(), pre_display);

    let mut input: Box<dyn Read> = match options.input {
        PreviewInput::Stdin => Box::new(io::stdin()),
        PreviewInput::File(path) => Box::new(
            File::open(&path)
                .map_err(|error| format!("cannot open `{}`: {error}", path.display()))?,
        ),
        PreviewInput::Demo => Box::new(io::Cursor::new(DEMO)),
    };
    let stdout = io::stdout();
    let mut display = stdout.lock();
    let mut chunk = [0_u8; 8192];

    loop {
        let count = input
            .read(&mut chunk)
            .map_err(|error| format!("cannot read preview input: {error}"))?;
        if count == 0 {
            break;
        }
        interceptor
            .feed(&chunk[..count], &mut display)
            .map_err(|error| error.to_string())?;
    }
    interceptor
        .finish(&mut display)
        .map_err(|error| error.to_string())?;
    display.flush().map_err(|error| error.to_string())?;
    Ok(0)
}

fn run_config(mut arguments: Vec<OsString>) -> Result<i32, String> {
    let action = arguments
        .first()
        .and_then(|argument| argument.to_str())
        .ok_or_else(|| "missing config action; use `paths`, `check`, or `show`".to_owned())?
        .to_owned();
    arguments.remove(0);

    let mut provenance = false;
    let mut options = ConfigOptions::default();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let text = argument
            .to_str()
            .ok_or_else(|| "config options must be valid UTF-8".to_owned())?;
        match text {
            "--config" => options.explicit_path = Some(next_path(&mut iterator, "--config")?),
            "--profile" => options.profile = Some(next_string(&mut iterator, "--profile")?),
            "--no-config" => options.no_config = true,
            "--provenance" if action == "show" => provenance = true,
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(0);
            }
            option => return Err(format!("unknown config option `{option}`")),
        }
    }

    let manager = ConfigManager::default();
    let environment = ConfigEnvironment::from_process();
    match action.as_str() {
        "paths" => {
            for source in manager.candidate_paths(&options.request(), &environment) {
                let origin = match source.origin {
                    ConfigOrigin::User => "user",
                    ConfigOrigin::Environment => "environment",
                    ConfigOrigin::Explicit => "explicit",
                    ConfigOrigin::Project => "project",
                };
                let trust = match source.trust {
                    ConfigTrust::UserOwned => "user-owned",
                    ConfigTrust::ExplicitlySelected => "explicitly-selected",
                    ConfigTrust::TrustedProject => "trusted-project",
                    ConfigTrust::UntrustedProject => "untrusted-project-not-loaded",
                };
                println!(
                    "{origin}\t{trust}\t{}\t{}",
                    if source.path.is_file() { "present" } else { "missing" },
                    source.path.display()
                );
            }
            Ok(0)
        }
        "check" => {
            let loaded = manager
                .load(options.request(), environment)
                .map_err(|error| error.to_string())?;
            println!(
                "configuration ok: schema={} profile={} sources={}",
                loaded.config.schema_version,
                loaded.config.profile,
                loaded.provenance.sources.len()
            );
            Ok(0)
        }
        "show" => {
            let loaded = manager
                .load(options.request(), environment)
                .map_err(|error| error.to_string())?;
            print!("{}", loaded.effective_toml().map_err(|error| error.to_string())?);
            if provenance {
                eprintln!(
                    "{}",
                    loaded
                        .provenance_toml()
                        .map_err(|error| error.to_string())?
                );
            }
            Ok(0)
        }
        _ => Err(format!(
            "unknown config action `{action}`; use `paths`, `check`, or `show`"
        )),
    }
}

fn run_command(mut arguments: Vec<OsString>) -> Result<i32, String> {
    if arguments.is_empty() {
        return Err("missing command after `--`".to_owned());
    }

    // Configuration is fully parsed and validated before replacing the process. This keeps invalid
    // configuration from failing after a future PTY host has entered raw mode or spawned a child.
    ConfigManager::default()
        .load_from_process(ConfigRequest::default())
        .map_err(|error| error.to_string())?;

    let program = arguments.remove(0);
    let mut command = Command::new(&program);
    command.args(arguments);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        Err(format!(
            "cannot execute `{}`: {error}",
            program.to_string_lossy()
        ))
    }

    #[cfg(not(unix))]
    {
        let status = command
            .status()
            .map_err(|error| format!("cannot execute `{}`: {error}", program.to_string_lossy()))?;
        Ok(status.code().unwrap_or(1))
    }
}
