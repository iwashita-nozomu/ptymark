use std::ffi::{OsStr, OsString};

/// A child command whose executable and arguments remain separate values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChildCommand {
    program: OsString,
    arguments: Vec<OsString>,
}

impl ChildCommand {
    pub(crate) fn from_argv(
        argv: Vec<OsString>,
        missing_message: &str,
    ) -> Result<Self, String> {
        let mut arguments = argv.into_iter();
        let program = arguments
            .next()
            .ok_or_else(|| missing_message.to_owned())?;
        Ok(Self {
            program,
            arguments: arguments.collect(),
        })
    }

    pub(crate) fn program(&self) -> &OsStr {
        &self.program
    }

    pub(crate) fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    pub(crate) fn display_name(&self) -> String {
        self.program.to_string_lossy().into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::ChildCommand;
    use std::ffi::OsString;

    #[test]
    fn preserves_program_and_argument_boundaries() {
        let command = ChildCommand::from_argv(
            vec![
                OsString::from("tool"),
                OsString::from("--flag"),
                OsString::from("value with spaces"),
            ],
            "missing",
        )
        .expect("command");

        assert_eq!(command.program(), "tool");
        assert_eq!(
            command.arguments(),
            [
                OsString::from("--flag"),
                OsString::from("value with spaces"),
            ]
        );
    }

    #[test]
    fn rejects_an_empty_command_line() {
        let error = ChildCommand::from_argv(Vec::new(), "missing command")
            .expect_err("empty command");
        assert_eq!(error, "missing command");
    }
}
