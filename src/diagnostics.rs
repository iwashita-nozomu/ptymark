use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

pub const MAX_PUBLIC_DIAGNOSTIC_BYTES: usize = 4096;
pub const REDACTED_VALUE: &str = "<redacted>";

pub mod code {
    pub const CONFIG_INVALID: &str = "config.invalid";
    pub const INSTALL_STATE_MISSING: &str = "install.state_missing";
    pub const INSTALL_STATE_STALE: &str = "install.state_stale";
    pub const HOST_UNAVAILABLE: &str = "host.unavailable";
    pub const TERMINAL_REDIRECTED: &str = "terminal.redirected";
    pub const ENGINE_MISSING: &str = "engine.missing";
    pub const ENGINE_INCOMPATIBLE: &str = "engine.incompatible";
    pub const BROWSER_UNAVAILABLE: &str = "browser.unavailable";
    pub const PRESENTER_UNSUPPORTED: &str = "presenter.unsupported";
    pub const RENDER_FAILED: &str = "render.failed";
    pub const RENDER_PROCESS_EXIT: &str = "render.process_exit";
    pub const RENDER_TIMEOUT: &str = "render.timeout";
    pub const RENDER_OUTPUT_LIMIT: &str = "render.output_limit";
    pub const PRESENTATION_FALLBACK: &str = "presentation.fallback";
    pub const MODE_SOURCE: &str = "mode.source";
    pub const MODE_SAFE: &str = "mode.safe";
    pub const MODE_PRIVATE: &str = "mode.private";
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

impl DiagnosticSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticComponent {
    Configuration,
    Installation,
    Host,
    Terminal,
    Engine,
    Browser,
    Presenter,
    Render,
    Presentation,
    Mode,
}

impl DiagnosticComponent {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::Installation => "installation",
            Self::Host => "host",
            Self::Terminal => "terminal",
            Self::Engine => "engine",
            Self::Browser => "browser",
            Self::Presenter => "presenter",
            Self::Render => "render",
            Self::Presentation => "presentation",
            Self::Mode => "mode",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticStatus {
    Ready,
    Degraded,
    Unusable,
}

impl DiagnosticStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Unusable => "unusable",
        }
    }

    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Ready => 0,
            Self::Degraded => 10,
            Self::Unusable => 20,
        }
    }

    pub fn from_findings(findings: &[DiagnosticFinding]) -> Self {
        if findings
            .iter()
            .any(|finding| finding.severity == DiagnosticSeverity::Error)
        {
            Self::Unusable
        } else if findings
            .iter()
            .any(|finding| finding.severity == DiagnosticSeverity::Warning)
        {
            Self::Degraded
        } else {
            Self::Ready
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticEvidence {
    pub value: String,
    pub redacted: bool,
}

impl DiagnosticEvidence {
    pub fn visible(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            redacted: false,
        }
    }

    pub fn redacted(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            redacted: true,
        }
    }

    pub fn omitted() -> Self {
        Self::redacted(REDACTED_VALUE)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticFinding {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub component: DiagnosticComponent,
    pub summary: String,
    pub remedy: Option<String>,
    pub evidence: BTreeMap<String, DiagnosticEvidence>,
}

impl DiagnosticFinding {
    pub fn new(
        code: impl Into<String>,
        severity: DiagnosticSeverity,
        component: DiagnosticComponent,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity,
            component,
            summary: summary.into(),
            remedy: None,
            evidence: BTreeMap::new(),
        }
    }

    pub fn with_remedy(mut self, remedy: impl Into<String>) -> Self {
        self.remedy = Some(remedy.into());
        self
    }

    pub fn with_evidence(
        mut self,
        key: impl Into<String>,
        value: DiagnosticEvidence,
    ) -> Self {
        self.evidence.insert(key.into(), value);
        self
    }

    pub fn human_line(&self) -> String {
        let mut line = format!(
            "[{}] {} ({}): {}",
            self.severity.as_str(),
            self.code,
            self.component.as_str(),
            self.summary
        );
        if let Some(remedy) = &self.remedy {
            line.push_str(" Remedy: ");
            line.push_str(remedy);
        }
        line
    }
}

#[derive(Clone, Debug)]
pub struct Redactor {
    home: Option<PathBuf>,
    sensitive: Vec<String>,
    max_bytes: usize,
}

impl Default for Redactor {
    fn default() -> Self {
        Self::from_environment()
    }
}

impl Redactor {
    pub fn from_environment() -> Self {
        let home = env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from);
        Self {
            home,
            sensitive: Vec::new(),
            max_bytes: MAX_PUBLIC_DIAGNOSTIC_BYTES,
        }
    }

    pub fn with_home(home: Option<PathBuf>) -> Self {
        Self {
            home,
            sensitive: Vec::new(),
            max_bytes: MAX_PUBLIC_DIAGNOSTIC_BYTES,
        }
    }

    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes.max(1);
        self
    }

    pub fn add_sensitive(&mut self, value: impl Into<String>) {
        let value = value.into();
        if !value.is_empty() && !self.sensitive.iter().any(|existing| existing == &value) {
            self.sensitive.push(value);
            self.sensitive
                .sort_by_key(|candidate| std::cmp::Reverse(candidate.len()));
        }
    }

    pub fn public_text(&self, input: &[u8]) -> DiagnosticEvidence {
        let mut text = String::from_utf8_lossy(input).into_owned();
        let mut redacted = false;

        for sensitive in &self.sensitive {
            if text.contains(sensitive) {
                text = text.replace(sensitive, REDACTED_VALUE);
                redacted = true;
            }
        }
        if let Some(home) = self.home.as_ref().and_then(|path| path.to_str())
            && !home.is_empty()
            && text.contains(home)
        {
            text = text.replace(home, "~");
            redacted = true;
        }

        let sanitized = sanitize_controls(&text);
        let (value, truncated) = truncate_utf8(&sanitized, self.max_bytes);
        DiagnosticEvidence {
            value,
            redacted: redacted || truncated,
        }
    }

    pub fn public_path(&self, path: &Path) -> DiagnosticEvidence {
        if let Some(home) = &self.home
            && let Ok(relative) = path.strip_prefix(home)
        {
            let value = if relative.as_os_str().is_empty() {
                "~".to_owned()
            } else {
                format!("~/{}", relative.display())
            };
            return DiagnosticEvidence::redacted(sanitize_controls(&value));
        }
        self.public_text(path.to_string_lossy().as_bytes())
    }
}

pub fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1f}' => {
                use std::fmt::Write as _;
                let _ = write!(output, "\\u{:04x}", character as u32);
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn sanitize_controls(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(output, "\\u{{{:04x}}}", character as u32);
            }
            character => output.push(character),
        }
    }
    output
}

fn truncate_utf8(value: &str, max_bytes: usize) -> (String, bool) {
    if value.len() <= max_bytes {
        return (value.to_owned(), false);
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    let mut truncated = value[..end].to_owned();
    truncated.push('…');
    (truncated, true)
}

#[cfg(test)]
mod tests {
    use super::{
        DiagnosticComponent, DiagnosticFinding, DiagnosticSeverity, DiagnosticStatus, Redactor,
        REDACTED_VALUE, code, json_string,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn stable_codes_and_status_mapping_are_explicit() {
        let findings = vec![
            DiagnosticFinding::new(
                code::MODE_SAFE,
                DiagnosticSeverity::Info,
                DiagnosticComponent::Mode,
                "safe mode is active",
            ),
            DiagnosticFinding::new(
                code::ENGINE_MISSING,
                DiagnosticSeverity::Warning,
                DiagnosticComponent::Engine,
                "the optional Mermaid engine is unavailable",
            ),
        ];
        assert_eq!(DiagnosticStatus::from_findings(&findings), DiagnosticStatus::Degraded);
        assert_eq!(DiagnosticStatus::Degraded.exit_code(), 10);

        let unusable = [DiagnosticFinding::new(
            code::CONFIG_INVALID,
            DiagnosticSeverity::Error,
            DiagnosticComponent::Configuration,
            "configuration is invalid",
        )];
        assert_eq!(DiagnosticStatus::from_findings(&unusable), DiagnosticStatus::Unusable);
        assert_eq!(DiagnosticStatus::Unusable.exit_code(), 20);
    }

    #[test]
    fn redactor_removes_sensitive_text_home_paths_and_controls() {
        let mut redactor = Redactor::with_home(Some(PathBuf::from("/home/alice")));
        redactor.add_sensitive("secret-token");
        let value = redactor.public_text(
            b"source=secret-token path=/home/alice/private.md\x1b[31m\n",
        );
        assert!(value.redacted);
        assert!(value.value.contains(REDACTED_VALUE));
        assert!(value.value.contains("~/private.md"));
        assert!(!value.value.contains("secret-token"));
        assert!(!value.value.contains('\u{1b}'));
        assert!(value.value.contains("\\u{001b}"));
    }

    #[test]
    fn path_redaction_is_structured() {
        let redactor = Redactor::with_home(Some(PathBuf::from("/Users/example")));
        let value = redactor.public_path(Path::new("/Users/example/.config/ptymark/config.toml"));
        assert_eq!(value.value, "~/.config/ptymark/config.toml");
        assert!(value.redacted);
    }

    #[test]
    fn bounded_text_does_not_split_utf8() {
        let redactor = Redactor::with_home(None).with_max_bytes(5);
        let value = redactor.public_text("日本語".as_bytes());
        assert!(value.value.ends_with('…'));
        assert!(value.redacted);
    }

    #[test]
    fn json_strings_escape_control_data() {
        assert_eq!(json_string("a\n\"b\\c\u{1b}"), "\"a\\n\\\"b\\\\c\\u001b\"");
    }
}
