use ptymark::{
    BlockRenderer, FencedDetector, PreDisplayRenderer, PreviewRenderer, RenderContext,
    SourceRenderer,
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

COMMANDS:
    preview     render bounded Mermaid and block-math input before display
    demo        render a built-in sample through the same pre-display pipeline

PREVIEW OPTIONS:
    --source                 emit the original semantic source instead of a preview
    --strict                 fail instead of restoring source when rendering fails
    --color                  emit ANSI color in compatible renderers
    --max-buffer-bytes N     semantic buffer limit (default: 1048576)
    --terminal-width N       width hint for renderer backends
    -h, --help               print this help

GLOBAL OPTIONS:
    -h, --help               print this help
    -V, --version            print the version

EXAMPLES:
    ptymark -- zsh -l
    printf '$$\\nE = mc^2\\n$$\\n' | ptymark preview
    ptymark demo --color
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

#[derive(Debug)]
struct PreviewOptions {
    source_renderer: bool,
    strict: bool,
    color: bool,
    max_buffer_bytes: usize,
    terminal_width: Option<usize>,
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
            "missing command; use `ptymark -- COMMAND` or `ptymark preview`".to_owned()
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

    Err("`--help` cannot be combined with preview options or an input file".to_owned())
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
        max_buffer_bytes: 1024 * 1024,
        terminal_width: None,
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
            "--max-buffer-bytes" => {
                options.max_buffer_bytes =
                    next_positive_usize(&mut iterator, "--max-buffer-bytes")?;
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

fn next_positive_usize(
    iterator: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<usize, String> {
    let value = iterator
        .next()
        .ok_or_else(|| format!("missing value after `{option}`"))?
        .into_string()
        .map_err(|_| format!("value for `{option}` must be valid UTF-8"))?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("value for `{option}` must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("value for `{option}` must be greater than zero"));
    }
    Ok(parsed)
}

fn run_preview(options: PreviewOptions) -> Result<i32, String> {
    let renderer: Box<dyn BlockRenderer> = if options.source_renderer {
        Box::new(SourceRenderer)
    } else {
        Box::new(PreviewRenderer)
    };
    let detector = FencedDetector::new(options.max_buffer_bytes);
    let context = RenderContext {
        color: options.color,
        terminal_width: options.terminal_width,
    };
    let mut pre_display = PreDisplayRenderer::new(detector, renderer)
        .with_context(context)
        .strict(options.strict);

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
        pre_display
            .feed(&chunk[..count], &mut display)
            .map_err(|error| error.to_string())?;
    }
    pre_display
        .finish(&mut display)
        .map_err(|error| error.to_string())?;
    display.flush().map_err(|error| error.to_string())?;
    Ok(0)
}

fn run_command(mut arguments: Vec<OsString>) -> Result<i32, String> {
    if arguments.is_empty() {
        return Err("missing command after `--`".to_owned());
    }
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
