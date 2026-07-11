use super::{ConfigOptions, load_config};
use std::ffi::OsString;
use std::process::Command;

pub(super) fn run(mut arguments: Vec<OsString>, options: ConfigOptions) -> Result<i32, String> {
    if arguments.is_empty() {
        return Err("missing command after `--`".to_owned());
    }

    // Resolve configuration before a future PTY host enters raw mode or spawns a child. The
    // resolved snapshot affects only the pre-display pipeline; input, signals, resize forwarding,
    // child environment, working directory, and exit semantics remain owned by the transport.
    let _loaded = load_config(&options)?;

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
