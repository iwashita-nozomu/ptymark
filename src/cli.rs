mod command;
mod config_command;
mod preview;

use crate::{ConfigManager, ConfigRequest, LoadedConfig};
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process;

pub const HELP: &str = "\
ptymark — pre-display semantic renderer for terminal streams

USAGE:
    ptymark [CONFIG OPTIONS] -- COMMAND [ARG...]
    ptymark [CONFIG OPTIONS] preview [OPTIONS] [FILE|-]
    ptymark [CONFIG OPTIONS] demo [OPTIONS]
    ptymark config paths [CONFIG OPTIONS]
    ptymark config check [CONFIG OPTIONS]
    ptymark config show [CONFIG OPTIONS] [--provenance]

COMMANDS:
    preview     render bounded Mermaid and block-math input before display
    demo        render a built-in sample through the same pre-display pipeline
    config      inspect and validate the immutable session configuration

CONFIG OPTIONS:
    --config PATH            load an explicit configuration after the user config
    --profile NAME           select a named configuration profile
    --no-config              ignore user and environment configuration files
    --private                force a no-cache, source-redacted private session

PREVIEW OPTIONS:
    --source                 emit the original semantic source instead of a preview
    --strict                 fail instead of restoring source when rendering fails
    --color                  emit ANSI color in compatible renderers
    --max-buffer-bytes N     override the configured semantic buffer limit
    --terminal-width N       width hint for renderer backends
    --no-cache               disable the configured in-process render cache
    -h, --help               print this help

GLOBAL OPTIONS:
    -h, --help               print this help
    -V, --version            print the version

EXAMPLES:
    ptymark --profile interactive -- zsh -l
    printf '$$\\nE = mc^2\\n$$\\n' | ptymark preview
    ptymark --profile private preview --no-cache
    ptymark config check --config ./ptymark.toml
    ptymark config show --profile interactive --provenance
";

#[derive(Clone, Debug, Default)]
pub(crate) struct ConfigOptions {
    pub(crate) explicit_path: Option<PathBuf>,
    pub(crate) profile: Option<String>,
    pub(crate) no_config: bool,
    pub(crate) private: bool,
}

impl ConfigOptions {
    pub(crate) fn request(&self) -> Result<ConfigRequest, String> {
        if self.no_config && self.explicit_path.is_some() {
            return Err("`--no-config` cannot be combined with `--config`".to_owned());
        }
        Ok(ConfigRequest {
            explicit_path: self.explicit_path.clone(),
            profile: self.profile.clone(),
            no_config: self.no_config,
            ..ConfigRequest::default()
        })
    }

    pub(crate) fn set_config(&mut self, value: PathBuf) -> Result<(), String> {
        if self.explicit_path.replace(value).is_some() {
            return Err("`--config` may be specified only once".to_owned());
        }
        Ok(())
    }

    pub(crate) fn set_profile(&mut self, value: String) -> Result<(), String> {
        if self.profile.replace(value).is_some() {
            return Err("`--profile` may be specified only once".to_owned());
        }
        Ok(())
    }
}

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

    let global_config = parse_leading_config_options(&mut arguments)?;
    let first = arguments
        .first()
        .and_then(|argument| argument.to_str())
        .ok_or_else(|| {
            "missing command; use `ptymark -- COMMAND`, `ptymark preview`, or `ptymark config`"
                .to_owned()
        })?
        .to_owned();

    match first.as_str() {
        "preview" => {
            arguments.remove(0);
            if subcommand_help_requested(&arguments)? {
                print!("{HELP}");
                return Ok(0);
            }
            preview::run(arguments, preview::PreviewInput::Stdin, global_config)
        }
        "demo" => {
            arguments.remove(0);
            if subcommand_help_requested(&arguments)? {
                print!("{HELP}");
                return Ok(0);
            }
            preview::run(arguments, preview::PreviewInput::Demo, global_config)
        }
        "config" => {
            arguments.remove(0);
            config_command::run(arguments, global_config)
        }
        "--" => {
            arguments.remove(0);
            command::run(arguments, global_config)
        }
        option if option.starts_with('-') => Err(format!("unknown option `{option}`")),
        _ => Err("commands must be introduced with `--`; example: `ptymark -- zsh -l`".to_owned()),
    }
}

fn parse_leading_config_options(arguments: &mut Vec<OsString>) -> Result<ConfigOptions, String> {
    let mut options = ConfigOptions::default();
    loop {
        let Some(option) = arguments.first().and_then(|value| value.to_str()) else {
            break;
        };
        match option {
            "--config" => {
                arguments.remove(0);
                let value = arguments
                    .first()
                    .cloned()
                    .ok_or_else(|| "missing value after `--config`".to_owned())?;
                arguments.remove(0);
                options.set_config(PathBuf::from(value))?;
            }
            "--profile" => {
                arguments.remove(0);
                let value = arguments
                    .first()
                    .cloned()
                    .ok_or_else(|| "missing value after `--profile`".to_owned())?;
                arguments.remove(0);
                let value = value
                    .into_string()
                    .map_err(|_| "value for `--profile` must be valid UTF-8".to_owned())?;
                if value.is_empty() {
                    return Err("value for `--profile` cannot be empty".to_owned());
                }
                options.set_profile(value)?;
            }
            "--no-config" => {
                arguments.remove(0);
                options.no_config = true;
            }
            "--private" => {
                arguments.remove(0);
                options.private = true;
            }
            _ => break,
        }
    }
    options.request()?;
    Ok(options)
}

pub(crate) fn subcommand_help_requested(arguments: &[OsString]) -> Result<bool, String> {
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

pub(crate) fn next_string(
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

pub(crate) fn next_path(
    iterator: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<PathBuf, String> {
    iterator
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing value after `{option}`"))
}

pub(crate) fn next_positive_usize(
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

pub(crate) fn load_config(options: &ConfigOptions) -> Result<LoadedConfig, String> {
    let mut loaded = ConfigManager::default()
        .load_from_process(options.request()?)
        .map_err(|error| error.to_string())?;
    if options.private || loaded.config.cache.private {
        loaded.config.apply_private_override();
    }
    Ok(loaded)
}
