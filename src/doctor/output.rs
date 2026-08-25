use super::DoctorReport;
use std::io::Write;
use std::path::Path;

impl DoctorReport {
    pub fn human(&self) -> String {
        let mut output = String::new();
        push_line(
            &mut output,
            format!("ptymark doctor: {}", self.status.as_str()),
        );
        push_line(
            &mut output,
            format!(
                "version: {} ({} {})",
                self.ptymark.version, self.ptymark.target_os, self.ptymark.target_arch
            ),
        );
        push_line(&mut output, format!("schema: {}", self.schema));
        push_line(
            &mut output,
            format!(
                "configuration: {} {}",
                self.configuration.state,
                self.configuration
                    .path
                    .as_ref()
                    .map_or("<built-in defaults>", |path| path.value.as_str())
            ),
        );
        push_line(
            &mut output,
            format!(
                "session: mode={} strict={} private={} cache={}",
                self.session.mode,
                self.session.strict,
                self.session.private,
                if self.session.cache_enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            ),
        );
        push_line(
            &mut output,
            format!(
                "terminal: stdin={} stdout={} host={} size={}",
                terminal_word(self.terminal.stdin_terminal),
                terminal_word(self.terminal.stdout_terminal),
                self.terminal.host,
                match (self.terminal.columns, self.terminal.rows) {
                    (Some(columns), Some(rows)) => format!("{columns}x{rows}"),
                    _ => "unknown".to_owned(),
                }
            ),
        );
        push_line(
            &mut output,
            format!(
                "installation: {} {}",
                self.installation.state,
                self.installation
                    .path
                    .as_ref()
                    .map_or("<unresolved>", |path| path.value.as_str())
            ),
        );
        for engine in &self.engines {
            push_line(
                &mut output,
                format!(
                    "engine {}: {} ({}, origin={}) runtime={}",
                    engine.role,
                    engine.backend,
                    engine.state,
                    engine.origin,
                    engine.runtime_state.unwrap_or("not-applicable")
                ),
            );
        }
        push_line(
            &mut output,
            format!(
                "presenter: {} ({}) runtime={}",
                self.presenter.backend,
                self.presenter.state,
                self.presenter.runtime_state.unwrap_or("not-applicable")
            ),
        );
        if self.findings.is_empty() {
            push_line(&mut output, "findings: none".to_owned());
        } else {
            push_line(&mut output, "findings:".to_owned());
            for finding in &self.findings {
                push_line(&mut output, format!("  {}", finding.human_line()));
            }
        }
        output
    }

    pub fn json(&self) -> String {
        let mut output = serde_json::to_string_pretty(self)
            .expect("the public-safe doctor report must be serializable");
        output.push('\n');
        output
    }

    pub fn write_support_report(&self, path: &Path) -> Result<(), String> {
        if path.exists() {
            return Err(format!(
                "support report `{}` already exists; choose a new path",
                path.display()
            ));
        }
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if !parent.is_dir() {
            return Err(format!(
                "support report directory `{}` does not exist",
                parent.display()
            ));
        }

        let mut temporary = tempfile::Builder::new()
            .prefix(".ptymark-support-")
            .tempfile_in(parent)
            .map_err(|error| format!("cannot create support report temporary file: {error}"))?;
        temporary
            .write_all(self.json().as_bytes())
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|error| format!("cannot write support report temporary file: {error}"))?;
        temporary.persist_noclobber(path).map_err(|error| {
            format!(
                "cannot publish support report `{}`: {}",
                path.display(),
                error.error
            )
        })?;
        Ok(())
    }
}

fn terminal_word(value: bool) -> &'static str {
    if value { "terminal" } else { "redirected" }
}

fn push_line(output: &mut String, line: String) {
    output.push_str(&line);
    output.push('\n');
}
