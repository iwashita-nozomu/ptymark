use crate::command::ChildCommand;
use crate::config::Config;
use crate::runtime::{PipelineFactory, PipelineOptions};
use crate::stream::PipelinePump;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub(crate) fn run(
    command: Vec<OsString>,
    config_path: Option<PathBuf>,
    profile: Option<String>,
    options: PipelineOptions,
) -> Result<i32, String> {
    let command = ChildCommand::from_argv(command, "missing child command after `run --`")?;
    let config = Config::load_profile(config_path.as_deref(), profile.as_deref())
        .map_err(|error| error.to_string())?;
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

    if let Err(error) = PipelinePump::standard().run_bounded_with_updates(
        &mut child_stdout,
        &mut display,
        &mut pipeline,
        |_| {},
        || terminate_child(&mut child),
    ) {
        terminate_child(&mut child);
        return Err(format!("cannot process child stdout: {error}"));
    }

    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for `{}`: {error}", command.display_name()))?;
    Ok(status.code().unwrap_or(1))
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
