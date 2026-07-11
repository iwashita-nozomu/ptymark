use super::{ConfigOptions, load_config, next_path, next_positive_usize, next_string};
use crate::{
    BlockRenderer, CacheBackend, CachePolicy, CoordinatedRenderer, DetectionMode,
    DisplayInterceptor, FallbackPolicy, FencedDetector, FencedDetectorOptions, MemoryArtifactCache,
    PassthroughDetector, PreDisplayRenderer, PresentationMode, RenderContext, SemanticDetector,
    SessionMode, SourceRenderer, TerminalOutputGate, stable_fingerprint,
};
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;

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

#[derive(Debug)]
pub(super) enum PreviewInput {
    Stdin,
    File(PathBuf),
    Demo,
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

pub(super) fn run(
    arguments: Vec<OsString>,
    default_input: PreviewInput,
    config: ConfigOptions,
) -> Result<i32, String> {
    run_preview(parse(arguments, default_input, config)?)
}

fn parse(
    arguments: Vec<OsString>,
    default_input: PreviewInput,
    config: ConfigOptions,
) -> Result<PreviewOptions, String> {
    let is_demo = matches!(default_input, PreviewInput::Demo);
    let mut options = PreviewOptions {
        source_renderer: false,
        strict: false,
        color: false,
        cache: None,
        max_buffer_bytes: None,
        terminal_width: None,
        config,
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
            "--private" => options.config.private = true,
            "--config" => {
                let path = next_path(&mut iterator, "--config")?;
                options.config.set_config(path)?;
            }
            "--profile" => {
                let profile = next_string(&mut iterator, "--profile")?;
                options.config.set_profile(profile)?;
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

    options.config.request()?;
    Ok(options)
}

fn run_preview(options: PreviewOptions) -> Result<i32, String> {
    let loaded = load_config(&options.config)?;
    let session = &loaded.config;

    let source_renderer = options.source_renderer
        || session.mode == SessionMode::Source
        || session.presentation.mode == PresentationMode::Source;
    let cache_forced_off = options.cache == Some(false) || source_renderer;
    let cache_enabled = match session.cache.backend {
        CacheBackend::None => false,
        CacheBackend::Memory => !cache_forced_off && !session.cache.private,
        CacheBackend::Disk | CacheBackend::Tiered if cache_forced_off => false,
        CacheBackend::Disk | CacheBackend::Tiered => {
            return Err(format!(
                "cache backend `{:?}` is configured but not implemented in the preview runtime; use `--no-cache` or backend = \"memory\"",
                session.cache.backend
            ));
        }
    };

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
            CoordinatedRenderer::preview(crate::NoopArtifactCache::default())
                .map_err(|error| error.to_string())?,
        )
    };

    let detection_enabled =
        session.mode != SessionMode::Bypass && session.detection.mode != DetectionMode::Off;
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

    let identity = loaded
        .fingerprint_material()
        .map_err(|error| error.to_string())?;
    let context = RenderContext {
        color: options.color,
        terminal_width: options.terminal_width,
        theme_fingerprint: 0,
        options_fingerprint: stable_fingerprint(&identity),
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
