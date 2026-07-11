use super::{ConfigOptions, HELP, load_config, next_path, next_string};
use crate::{RuntimeBuilder, RuntimeRequest};
use std::ffi::OsString;

pub(super) fn run(mut arguments: Vec<OsString>, mut options: ConfigOptions) -> Result<i32, String> {
    let action = arguments
        .first()
        .and_then(|argument| argument.to_str())
        .ok_or_else(|| "missing engine action; use `list` or `doctor`".to_owned())?
        .to_owned();
    arguments.remove(0);

    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let text = argument
            .to_str()
            .ok_or_else(|| "engine options must be valid UTF-8".to_owned())?;
        match text {
            "--config" => options.set_config(next_path(&mut iterator, "--config")?)?,
            "--profile" => options.set_profile(next_string(&mut iterator, "--profile")?)?,
            "--no-config" => options.no_config = true,
            "--private" => options.private = true,
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(0);
            }
            option => return Err(format!("unknown engine option `{option}`")),
        }
    }
    options.request()?;

    match action.as_str() {
        "list" => list(options),
        "doctor" => doctor(options),
        _ => Err(format!(
            "unknown engine action `{action}`; use `list` or `doctor`"
        )),
    }
}

fn build(options: &ConfigOptions) -> Result<crate::SessionRuntime, String> {
    let snapshot = load_config(options)?
        .into_snapshot(1)
        .map_err(|error| error.to_string())?;
    let mut request = RuntimeRequest::preview();
    request.disable_cache = true;
    RuntimeBuilder::default()
        .build(snapshot, request)
        .map_err(|error| error.to_string())
}

fn list(options: ConfigOptions) -> Result<i32, String> {
    let runtime = build(&options)?;
    for descriptor in &runtime.build_report().registered_engines {
        let kinds = descriptor
            .supported_kinds
            .iter()
            .map(|kind| kind.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let formats = descriptor
            .formats
            .iter()
            .map(|format| format.as_str())
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{}\t{}\t{}\t{}\t{:?}",
            descriptor.identity.id,
            descriptor.identity.version,
            kinds,
            formats,
            descriptor.execution_model
        );
    }
    Ok(0)
}

fn doctor(options: ConfigOptions) -> Result<i32, String> {
    let runtime = build(&options)?;
    let report = runtime.build_report();
    println!("runtime ok: {}", report.summary());
    println!("engine providers:");
    for provider in &report.engine_providers {
        println!("  ok\t{provider}");
    }
    println!("registered engines:");
    for descriptor in &report.registered_engines {
        println!(
            "  ok\t{}@{}\t{:?}",
            descriptor.identity.id, descriptor.identity.version, descriptor.execution_model
        );
    }
    if report.unavailable_engines.is_empty() {
        println!("unavailable optional engines: none");
    } else {
        println!("unavailable optional engines:");
        for unavailable in &report.unavailable_engines {
            println!("  optional\t{}\t{}", unavailable.id, unavailable.reason);
        }
    }
    if !report.warnings.is_empty() {
        println!("warnings:");
        for warning in &report.warnings {
            println!("  warn\t{warning}");
        }
    }
    Ok(0)
}
