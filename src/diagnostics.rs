use std::env;
use std::fmt;
use std::path::{Path, PathBuf};

pub const MAX_PUBLIC_EVIDENCE_CHARS: usize = 512;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FindingCode {
    ConfigInvalid,
    InstallStateMissing,
    InstallStateStale,
    HostUnavailable,
    TerminalRedirected,
    EngineMissing,
    EngineIncompatible,
    BrowserUnavailable,
    PresenterUnsupported,
    RenderProcessExit,
    RenderTimeout,
    RenderOutputLimit,
    PresentationFallback,
    ModeSource,
    ModeSafe,
    ModePrivate,
}

impl FindingCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigInvalid => "config.invalid",
            Self::InstallStateMissing => "install.state_missing",
            Self::InstallStateStale => "install.state_stale",
            Self::HostUnavailable => "host.unavailable",
            Self::TerminalRedirected => "terminal.redirected",
            Self::EngineMissing => "engine.missing",
            Self::EngineIncompatible => "engine.incompatible",
            Self::BrowserUnavailable => "browser.unavailable",
            Self::PresenterUnsupported => "presenter.unsupported",
            Self::RenderProcessExit => "render.process_exit",
            Self::RenderTimeout => "render.timeout",
            Self::RenderOutputLimit => "render.output_limit",
            Self::PresentationFallback => "presentation.fallback",
            Self::ModeSource => "mode.source",
            Self::ModeSafe => "mode.safe",
            Self::ModePrivate => "mode.private",
        }
    }
}

impl fmt::Display for FindingCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticFinding {
    pub code: FindingCode,
    pub severity: Severity,
    pub component: String,
    pub summary: String,
    pub remedy: String,
    pub evidence: Option<String>,
}

impl DiagnosticFinding {
    pub fn new(
        code: FindingCode,
        severity: Severity,
        component: impl Into<String>,
        summary: impl Into<String>,
        remedy: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity,
            component: component.into(),
            summary: summary.into(),
            remedy: remedy.into(),
            evidence: None,
        }
    }

    pub fn with_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.evidence = Some(evidence.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct Redactor {
    home: Option<PathBuf>,
    cwd: Option<PathBuf>,
    temp: PathBuf,
}

impl Default for Redactor {
    fn default() -> Self {
        Self::from_environment()
    }
}

impl Redactor {
    pub fn from_environment() -> Self {
        Self {
            home: env::var_os("HOME")
                .or_else(|| env::var_os("USERPROFILE"))
                .map(PathBuf::from),
            cwd: env::current_dir().ok(),
            temp: env::temp_dir(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_roots(
        home: Option<PathBuf>,
        cwd: Option<PathBuf>,
        temp: PathBuf,
    ) -> Self {
        Self { home, cwd, temp }
    }

    pub fn path(&self, path: &Path) -> String {
        if let Some(home) = self.home.as_deref()
            && let Ok(relative) = path.strip_prefix(home)
        {
            return joined_label("~", relative);
        }
        if let Ok(relative) = path.strip_prefix(&self.temp) {
            return joined_label("<temp>", relative);
        }
        if let Some(cwd) = self.cwd.as_deref()
            && let Ok(relative) = path.strip_prefix(cwd)
        {
            return joined_label("<cwd>", relative);
        }
        if path.is_absolute() {
            return path.file_name().map_or_else(
                || "<absolute>".to_owned(),
                |name| format!("<absolute>/{}", sanitize_lossy(name.to_string_lossy().as_bytes(), 160)),
            );
        }
        sanitize_lossy(path.as_os_str().to_string_lossy().as_bytes(), 240)
    }

    pub fn text(&self, bytes: &[u8]) -> String {
        sanitize_lossy(bytes, MAX_PUBLIC_EVIDENCE_CHARS)
    }

    pub fn bounded_text(&self, text: &str, limit: usize) -> String {
        sanitize_lossy(text.as_bytes(), limit)
    }

    pub const fn sensitive_value(&self) -> &'static str {
        "[redacted]"
    }
}

fn joined_label(label: &str, relative: &Path) -> String {
    if relative.as_os_str().is_empty() {
        label.to_owned()
    } else {
        format!(
            "{label}/{}",
            sanitize_lossy(relative.as_os_str().to_string_lossy().as_bytes(), 220)
        )
    }
}

pub fn sanitize_lossy(bytes: &[u8], limit: usize) -> String {
    let source = String::from_utf8_lossy(bytes);
    let mut output = String::new();
    let mut count = 0_usize;
    for character in source.chars() {
        if count >= limit {
            output.push('…');
            break;
        }
        match character {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{fffd}' => output.push_str("<invalid-utf8>"),
            value if value.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(output, "\\u{{{:x}}}", value as u32);
            }
            value => output.push(value),
        }
        count = count.saturating_add(1);
    }
    output
}

pub fn json_escape(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(8));
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            value if value <= '\u{1f}' => {
                use std::fmt::Write as _;
                let _ = write!(output, "\\u{:04x}", value as u32);
            }
            value => output.push(value),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{FindingCode, Redactor, Severity, json_escape, sanitize_lossy};
    use std::path::{Path, PathBuf};

    #[test]
    fn stable_codes_and_severities_are_machine_readable() {
        assert_eq!(FindingCode::RenderTimeout.as_str(), "render.timeout");
        assert_eq!(Severity::Warning.as_str(), "warning");
    }

    #[test]
    fn paths_are_public_safe() {
        let redactor = Redactor::with_roots(
            Some(PathBuf::from("/home/alice")),
            Some(PathBuf::from("/work/project")),
            PathBuf::from("/tmp"),
        );
        assert_eq!(redactor.path(Path::new("/home/alice/.config/ptymark/config.toml")), "~/.config/ptymark/config.toml");
        assert_eq!(redactor.path(Path::new("/work/project/examples/ptymark.toml")), "<cwd>/examples/ptymark.toml");
        assert_eq!(redactor.path(Path::new("/tmp/ptymark/report.json")), "<temp>/ptymark/report.json");
        assert_eq!(redactor.path(Path::new("/opt/private/bin/mmdc")), "<absolute>/mmdc");
    }

    #[test]
    fn control_and_invalid_bytes_are_escaped() {
        let text = sanitize_lossy(b"ok\x1b]8;;secret\x07\xff\n", 100);
        assert!(!text.contains('\x1b'));
        assert!(!text.contains('\x07'));
        assert!(text.contains("\\u{1b}"));
        assert!(text.contains("<invalid-utf8>"));
        assert!(text.ends_with("\\n"));
    }

    #[test]
    fn json_escaping_is_deterministic() {
        assert_eq!(json_escape("a\"b\\c\n"), "a\\\"b\\\\c\\n");
    }
}
