use super::{ConfigOptions, HELP, load_config, next_path, next_string};
use crate::{ConfigEnvironment, ConfigManager, ConfigOrigin, ConfigTrust};
use std::ffi::OsString;

pub(super) fn run(
    mut arguments: Vec<OsString>,
    mut options: ConfigOptions,
) -> Result<i32, String> {
    let action = arguments
        .first()
        .and_then(|argument| argument.to_str())
        .ok_or_else(|| "missing config action; use `paths`, `check`, or `show`".to_owned())?
        .to_owned();
    arguments.remove(0);

    let mut provenance = false;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let text = argument
            .to_str()
            .ok_or_else(|| "config options must be valid UTF-8".to_owned())?;
        match text {
            "--config" => options.set_config(next_path(&mut iterator, "--config")?)?,
            "--profile" => options.set_profile(next_string(&mut iterator, "--profile")?)?,
            "--no-config" => options.no_config = true,
            "--private" => options.private = true,
            "--provenance" if action == "show" => provenance = true,
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(0);
            }
            option => return Err(format!("unknown config option `{option}`")),
        }
    }
    options.request()?;

    match action.as_str() {
        "paths" => show_paths(options),
        "check" => check(options),
        "show" => show(options, provenance),
        _ => Err(format!(
            "unknown config action `{action}`; use `paths`, `check`, or `show`"
        )),
    }
}

fn show_paths(options: ConfigOptions) -> Result<i32, String> {
    let manager = ConfigManager::default();
    let environment = ConfigEnvironment::from_process();
    for source in manager.candidate_paths(&options.request()?, &environment) {
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
            if source.path.is_file() {
                "present"
            } else {
                "missing"
            },
            source.path.display()
        );
    }
    Ok(0)
}

fn check(options: ConfigOptions) -> Result<i32, String> {
    let loaded = load_config(&options)?;
    println!(
        "configuration ok: schema={} profile={} sources={}",
        loaded.config.schema_version,
        loaded.config.profile,
        loaded.provenance.sources.len()
    );
    Ok(0)
}

fn show(options: ConfigOptions, provenance: bool) -> Result<i32, String> {
    let loaded = load_config(&options)?;
    print!(
        "{}",
        loaded.effective_toml().map_err(|error| error.to_string())?
    );
    if provenance {
        // Keep the normalized TOML on stdout machine-readable. Provenance is a separate diagnostic
        // stream until a versioned JSON envelope is implemented under issue #15.
        eprintln!(
            "{}",
            loaded
                .provenance_toml()
                .map_err(|error| error.to_string())?
        );
    }
    Ok(0)
}
