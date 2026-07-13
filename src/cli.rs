use crate::cli_args::{apply_render_option, next_path, next_value, set_once};
use crate::config::Config;
use crate::engine::check_configured_engines;
use crate::install::{
    EnginePreference, InstallRequest, InstallState, Installer, PathProgramResolver,
    PresenterPreference, default_install_state_path,
};
use crate::runtime::{PipelineFactory, PipelineOptions};
use crate::stream::PipelinePump;
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;

pub const HELP: &str = "\
ptymark — pre-display renderer for terminal streams

USAGE:
    ptymark [--config PATH] preview [OPTIONS] [FILE|-]
    ptymark [--config PATH] run [OPTIONS] -- COMMAND [ARG...]
    ptymark [--config PATH] config check
    ptymark [--config PATH] config show
    ptymark [--config PATH] engine check
    ptymark [--config PATH] install resolve [INSTALL OPTIONS]
    ptymark install status [--state PATH]
    ptymark [--config PATH] -- COMMAND [ARG...]

RENDER OPTIONS:
    --source              keep complete semantic blocks as source
    --strict              fail instead of restoring source after renderer errors
    --no-cache            disable the in-memory render cache
    --color               allow ANSI color in terminal renderers
    --columns N           renderer width hint
    --config PATH         use an explicit ptymark TOML file
    -h, --help            print this help

COMMAND MODES:
    run -- COMMAND        pipe-oriented stdout filtering for batch/log tools
    -- COMMAND            native PTY/ConPTY host for shells, Codex, and TUIs

INSTALL OPTIONS:
    --config PATH         write or update this user configuration
    --state PATH          write the resolved installation snapshot here
    --mermaid VALUE       keep | auto | preview | source | EXECUTABLE
    --math VALUE          keep | auto | preview | source | EXECUTABLE
    --presenter VALUE     keep | auto | EXECUTABLE
    --reset               start from built-in defaults instead of preserving config
    --dry-run             print the plan without writing files

GLOBAL OPTIONS:
    --config PATH         validate and use an explicit ptymark TOML file
    -h, --help            print this help
    -V, --version         print the version

EXAMPLES:
    bash scripts/install.sh
    ptymark install resolve
    ptymark install resolve --mermaid /opt/homebrew/bin/mmdc
    ptymark install status
    printf '$$\nE = mc^2\n$$\n' | ptymark preview
    ptymark preview --source README.md
    ptymark run -- command-that-prints-markdown
    ptymark -- \"$SHELL\" -l
    ptymark -- codex
";

pub fn main_entry() -> ! {
    if let Some(result) = crate::managed_launcher::run_if_managed_alias() {
        match result {
            Ok(code) => process::exit(code),
            Err(message) => {
                eprintln!("ptymark managed launcher: {message}");
                process::exit(2);
            }
        }
    }
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
        .ok_or_else(|| {
            "missing command; use `preview`, `run`, `config`, `engine`, `install`, or `-- COMMAND`"
                .to_owned()
        })?
        .to_owned();
    arguments.remove(0);

    match command.as_str() {
        "preview" => run_preview(arguments, config_path),
        "run" => crate::filtered_run::run(arguments, config_path),
        "config" => run_config(arguments, config_path),
        "engine" => run_engine(arguments, config_path),
        "install" => run_install(arguments, config_path),
        "--" => crate::interactive::run(arguments, config_path),
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
        set_once(config_path, PathBuf::from(path), "--config")?;
    }
    Ok(())
}

fn next_engine_preference(
    iterator: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<EnginePreference, String> {
    let value = next_value(iterator, option)?;
    Ok(match value.to_str() {
        Some("keep") => EnginePreference::Keep,
        Some("auto") => EnginePreference::Auto,
        Some("preview") => EnginePreference::Preview,
        Some("source") => EnginePreference::Source,
        _ => EnginePreference::External(PathBuf::from(value)),
    })
}

fn next_presenter_preference(
    iterator: &mut impl Iterator<Item = OsString>,
) -> Result<PresenterPreference, String> {
    let value = next_value(iterator, "--presenter")?;
    Ok(match value.to_str() {
        Some("keep") => PresenterPreference::Keep,
        Some("auto") => PresenterPreference::Auto,
        Some("preview" | "source") => {
            return Err("`--presenter` accepts keep, auto, or an executable path".to_owned());
        }
        _ => PresenterPreference::Program(PathBuf::from(value)),
    })
}

fn run_preview(arguments: Vec<OsString>, mut config_path: Option<PathBuf>) -> Result<i32, String> {
    let mut options = PipelineOptions::default();
    let mut input_path = None;
    let mut iterator = arguments.into_iter();

    while let Some(argument) = iterator.next() {
        let text = argument
            .to_str()
            .ok_or_else(|| "preview options must be valid UTF-8".to_owned())?;
        if apply_render_option(text, &mut iterator, &mut options, &mut config_path)? {
            continue;
        }
        match text {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(0);
            }
            "-" => {
                set_once(&mut input_path, PathBuf::from("-"), "preview input")?;
            }
            option if option.starts_with('-') => {
                return Err(format!("unknown preview option `{option}`"));
            }
            _ => {
                set_once(&mut input_path, PathBuf::from(argument), "preview input")?;
            }
        }
    }

    let config = Config::load(config_path.as_deref()).map_err(|error| error.to_string())?;
    let mut pipeline = PipelineFactory::new(&config).build(options);
    let mut input: Box<dyn Read> = match input_path {
        Some(path) if path != *"-" => Box::new(
            File::open(&path)
                .map_err(|error| format!("cannot open `{}`: {error}", path.display()))?,
        ),
        _ => Box::new(io::stdin()),
    };
    let stdout = io::stdout();
    let mut display = stdout.lock();

    PipelinePump::standard()
        .run(input.as_mut(), &mut display, &mut pipeline)
        .map_err(|error| format!("cannot process preview input: {error}"))?;
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

    parse_subcommand_config(arguments, &mut config_path, "config")?;
    let config = Config::load(config_path.as_deref()).map_err(|error| error.to_string())?;
    match action.as_str() {
        "check" => {
            println!(
                "configuration ok: schema={} renderer={:?} mermaid={} math={}",
                config.schema_version,
                config.rendering.mode,
                config.engines.mermaid.backend.as_str(),
                config.engines.math.backend.as_str()
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

fn run_engine(
    mut arguments: Vec<OsString>,
    mut config_path: Option<PathBuf>,
) -> Result<i32, String> {
    let action = arguments
        .first()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "missing engine action; use `check`".to_owned())?
        .to_owned();
    arguments.remove(0);

    parse_subcommand_config(arguments, &mut config_path, "engine")?;
    if action != "check" {
        return Err(format!("unknown engine action `{action}`; use `check`"));
    }

    let config = Config::load(config_path.as_deref()).map_err(|error| error.to_string())?;
    for check in check_configured_engines(&config.engines).map_err(|error| error.to_string())? {
        println!("{}", check.display_line());
    }
    Ok(0)
}

fn run_install(mut arguments: Vec<OsString>, config_path: Option<PathBuf>) -> Result<i32, String> {
    let action = arguments
        .first()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "missing install action; use `resolve` or `status`".to_owned())?
        .to_owned();
    arguments.remove(0);

    match action.as_str() {
        "resolve" => run_install_resolve(arguments, config_path),
        "status" => run_install_status(arguments),
        _ => Err(format!(
            "unknown install action `{action}`; use `resolve` or `status`"
        )),
    }
}

fn run_install_resolve(
    arguments: Vec<OsString>,
    mut config_path: Option<PathBuf>,
) -> Result<i32, String> {
    let mut state_path = None;
    let mut mermaid = EnginePreference::Keep;
    let mut math = EnginePreference::Keep;
    let mut presenter = PresenterPreference::Keep;
    let mut reset = false;
    let mut dry_run = false;
    let mut iterator = arguments.into_iter();

    while let Some(argument) = iterator.next() {
        let text = argument
            .to_str()
            .ok_or_else(|| "install options must be valid UTF-8".to_owned())?;
        match text {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(0);
            }
            "--config" => set_once(
                &mut config_path,
                next_path(&mut iterator, "--config")?,
                "--config",
            )?,
            "--state" => set_once(
                &mut state_path,
                next_path(&mut iterator, "--state")?,
                "--state",
            )?,
            "--mermaid" => mermaid = next_engine_preference(&mut iterator, "--mermaid")?,
            "--math" => math = next_engine_preference(&mut iterator, "--math")?,
            "--presenter" => presenter = next_presenter_preference(&mut iterator)?,
            "--reset" => reset = true,
            "--dry-run" => dry_run = true,
            option => return Err(format!("unknown install option `{option}`")),
        }
    }

    let config_path = config_path
        .map(Ok)
        .unwrap_or_else(Config::user_config_path)
        .map_err(|error| error.to_string())?;
    let state_path = state_path
        .map(Ok)
        .unwrap_or_else(default_install_state_path)
        .map_err(|error| error.to_string())?;
    let mut request = InstallRequest::new(config_path, state_path);
    request.mermaid = mermaid;
    request.math = math;
    request.presenter = presenter;
    request.reset = reset;

    let plan = Installer::new(PathProgramResolver)
        .plan(&request)
        .map_err(|error| error.to_string())?;
    if dry_run {
        println!("# resolved ptymark configuration");
        print!(
            "{}",
            plan.config.to_toml().map_err(|error| error.to_string())?
        );
        println!("# resolved installation state");
        print!(
            "{}",
            plan.state.to_toml().map_err(|error| error.to_string())?
        );
    } else {
        plan.apply().map_err(|error| error.to_string())?;
        for line in plan.summary_lines() {
            println!("{line}");
        }
    }
    Ok(0)
}

fn run_install_status(arguments: Vec<OsString>) -> Result<i32, String> {
    let mut state_path = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        match argument.to_str() {
            Some("-h" | "--help") => {
                print!("{HELP}");
                return Ok(0);
            }
            Some("--state") => set_once(
                &mut state_path,
                next_path(&mut iterator, "--state")?,
                "--state",
            )?,
            Some(option) => return Err(format!("unknown install status option `{option}`")),
            None => return Err("install status options must be valid UTF-8".to_owned()),
        }
    }

    let state_path = state_path
        .map(Ok)
        .unwrap_or_else(default_install_state_path)
        .map_err(|error| error.to_string())?;
    let state = InstallState::load(&state_path).map_err(|error| error.to_string())?;
    println!("state\t{}", state_path.display());
    for line in state.status_lines(&PathProgramResolver) {
        println!("{line}");
    }
    Ok(0)
}

fn parse_subcommand_config(
    arguments: Vec<OsString>,
    config_path: &mut Option<PathBuf>,
    command: &str,
) -> Result<(), String> {
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        match argument.to_str() {
            Some("--config") => set_once(
                config_path,
                next_path(&mut iterator, "--config")?,
                "--config",
            )?,
            Some("-h" | "--help") => {
                print!("{HELP}");
                return Ok(());
            }
            Some(option) => return Err(format!("unknown {command} option `{option}`")),
            None => return Err(format!("{command} options must be valid UTF-8")),
        }
    }
    Ok(())
}
