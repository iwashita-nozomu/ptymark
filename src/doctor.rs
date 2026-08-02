use crate::config::{Config, RenderMode};
use crate::diagnostics::{
    DiagnosticComponent, DiagnosticEvidence, DiagnosticFinding, DiagnosticSeverity,
    DiagnosticStatus, Redactor, code,
};
use crate::engine::resolve_executable;
use crate::install::{InstallState, default_install_state_path};
use crate::managed_launcher::inspect_managed_alias;
use crate::runtime::PipelineOptions;
use serde::Serialize;
use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

pub const DOCTOR_SCHEMA: &str = "ptymark.doctor.v1";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DoctorRequest {
    pub config_path: Option<PathBuf>,
    pub pipeline: PipelineOptions,
}

/// Public-safe diagnostic model. Raw semantic source, child environment, and
/// unrestricted renderer stderr never enter this serializable type.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DoctorReport {
    pub schema: &'static str,
    pub status: DiagnosticStatus,
    pub ptymark: PtymarkReport,
    pub configuration: ConfigurationReport,
    pub installation: InstallationReport,
    pub session: SessionReport,
    pub terminal: TerminalReport,
    pub engines: Vec<EngineReport>,
    pub presenter: PresenterReport,
    pub recent_runtime: RecentRuntimeReport,
    pub findings: Vec<DiagnosticFinding>,
    pub redaction: RedactionReport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PtymarkReport {
    pub version: String,
    pub target_os: &'static str,
    pub target_arch: &'static str,
    pub config_schema: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConfigurationReport {
    pub selection: &'static str,
    pub path: Option<DiagnosticEvidence>,
    pub state: &'static str,
    pub schema_version: Option<u32>,
    pub strict: bool,
    pub rendering_mode: &'static str,
    pub cache_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstallationReport {
    pub path: Option<DiagnosticEvidence>,
    pub state: &'static str,
    pub installed_version: Option<String>,
    pub component_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SessionReport {
    pub mode: &'static str,
    pub private: bool,
    pub strict: bool,
    pub cache_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TerminalReport {
    pub stdin_terminal: bool,
    pub stdout_terminal: bool,
    pub columns: Option<u16>,
    pub rows: Option<u16>,
    pub host: &'static str,
    pub transport_hints: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EngineReport {
    pub role: &'static str,
    pub backend: String,
    pub origin: &'static str,
    pub state: &'static str,
    pub browser_state: Option<&'static str>,
    pub configured_path: Option<DiagnosticEvidence>,
    pub resolved_path: Option<DiagnosticEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PresenterReport {
    pub required: bool,
    pub backend: &'static str,
    pub state: &'static str,
    pub configured_path: Option<DiagnosticEvidence>,
    pub resolved_path: Option<DiagnosticEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RecentRuntimeReport {
    pub state: &'static str,
    pub finding_code: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RedactionReport {
    pub public_safe_default: bool,
    pub semantic_source: &'static str,
    pub child_environment: &'static str,
    pub renderer_stderr: &'static str,
    pub home_paths: &'static str,
}

impl DoctorReport {
    pub fn collect(request: DoctorRequest) -> Self {
        let redactor = Redactor::default();
        let mut findings = Vec::new();
        let selection = if request.config_path.is_some() {
            "explicit"
        } else {
            "default"
        };
        let selected_config_path = match request.config_path.clone() {
            Some(path) => Some(path),
            None => Config::user_config_path().ok(),
        };

        let (config, config_state, schema_version) = load_configuration(
            request.config_path.as_deref(),
            selected_config_path.as_deref(),
            &mut findings,
        );
        let strict = request.pipeline.strict || config.rendering.strict;
        let source = request.pipeline.source || config.rendering.mode == RenderMode::Source;
        let safe = request.pipeline.safe;
        let private = request.pipeline.private;
        let mode = if safe {
            "safe"
        } else if source {
            "source"
        } else {
            "configured"
        };
        let cache_enabled = config.cache.enabled
            && !request.pipeline.no_cache
            && !request.pipeline.private
            && !safe
            && !source;

        if safe {
            findings.push(
                DiagnosticFinding::new(
                    code::MODE_SAFE,
                    DiagnosticSeverity::Info,
                    DiagnosticComponent::Mode,
                    "safe mode bypasses semantic detection and external rendering",
                )
                .with_remedy("remove --safe only when semantic rendering is desired"),
            );
        } else if source {
            findings.push(
                DiagnosticFinding::new(
                    code::MODE_SOURCE,
                    DiagnosticSeverity::Info,
                    DiagnosticComponent::Mode,
                    "source mode detects complete blocks but displays exact source",
                )
                .with_remedy("remove --source or select preview rendering to render blocks"),
            );
        }
        if private {
            findings.push(
                DiagnosticFinding::new(
                    code::MODE_PRIVATE,
                    DiagnosticSeverity::Info,
                    DiagnosticComponent::Mode,
                    "private mode disables caches and persistent diagnostic artifacts",
                )
                .with_remedy("no action is required; this is an intentional privacy mode"),
            );
        }

        let installation =
            inspect_installation(selected_config_path.as_deref(), &redactor, &mut findings);
        let (engines, presenter) =
            inspect_engines(&config, strict, safe || source, &redactor, &mut findings);
        let terminal = inspect_terminal(&mut findings);
        let configuration = ConfigurationReport {
            selection,
            path: selected_config_path
                .as_deref()
                .map(|path| redactor.public_path(path)),
            state: config_state,
            schema_version,
            strict,
            rendering_mode: match config.rendering.mode {
                RenderMode::Preview => "preview",
                RenderMode::Source => "source",
            },
            cache_enabled,
        };

        findings.sort_by(|left, right| {
            left.code
                .cmp(&right.code)
                .then_with(|| left.component.cmp(&right.component))
                .then_with(|| left.summary.cmp(&right.summary))
        });
        let status = DiagnosticStatus::from_findings(&findings);

        Self {
            schema: DOCTOR_SCHEMA,
            status,
            ptymark: PtymarkReport {
                version: env!("CARGO_PKG_VERSION").to_owned(),
                target_os: env::consts::OS,
                target_arch: env::consts::ARCH,
                config_schema: crate::CONFIG_SCHEMA_VERSION,
            },
            configuration,
            installation,
            session: SessionReport {
                mode,
                private,
                strict,
                cache_enabled,
            },
            terminal,
            engines,
            presenter,
            recent_runtime: RecentRuntimeReport {
                state: "unavailable",
                finding_code: None,
            },
            findings,
            redaction: RedactionReport {
                public_safe_default: true,
                semantic_source: "excluded",
                child_environment: "excluded",
                renderer_stderr: "bounded-and-sanitized",
                home_paths: "abbreviated",
            },
        }
    }

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
                    "engine {}: {} ({}, origin={})",
                    engine.role, engine.backend, engine.state, engine.origin
                ),
            );
        }
        push_line(
            &mut output,
            format!(
                "presenter: {} ({})",
                self.presenter.backend, self.presenter.state
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

fn load_configuration(
    explicit: Option<&Path>,
    selected_path: Option<&Path>,
    findings: &mut Vec<DiagnosticFinding>,
) -> (Config, &'static str, Option<u32>) {
    let result = match explicit {
        Some(path) => Config::load_exact(path),
        None => Config::load(None),
    };
    match result {
        Ok(config) => {
            let state = if explicit.is_some() || selected_path.is_some_and(Path::is_file) {
                "valid"
            } else {
                "built-in-defaults"
            };
            let schema = Some(config.schema_version);
            (config, state, schema)
        }
        Err(_error) => {
            findings.push(
                DiagnosticFinding::new(
                    code::CONFIG_INVALID,
                    DiagnosticSeverity::Error,
                    DiagnosticComponent::Configuration,
                    "the selected configuration cannot be used",
                )
                .with_remedy(
                    "run `ptymark config check --config PATH` and correct the reported file",
                )
                .with_evidence("error", DiagnosticEvidence::omitted()),
            );
            (Config::default(), "invalid", None)
        }
    }
}

fn inspect_installation(
    selected_config_path: Option<&Path>,
    redactor: &Redactor,
    findings: &mut Vec<DiagnosticFinding>,
) -> InstallationReport {
    let state_path = match default_install_state_path() {
        Ok(path) => path,
        Err(_error) => {
            findings.push(
                DiagnosticFinding::new(
                    code::INSTALL_STATE_MISSING,
                    DiagnosticSeverity::Info,
                    DiagnosticComponent::Installation,
                    "the platform installation-state path is unavailable",
                )
                .with_remedy(
                    "this is expected for a source build; package users can rerun the installer",
                )
                .with_evidence("error", DiagnosticEvidence::omitted()),
            );
            return InstallationReport {
                path: None,
                state: "unavailable",
                installed_version: None,
                component_count: 0,
            };
        }
    };
    let redacted_path = redactor.public_path(&state_path);
    if !state_path.is_file() {
        findings.push(
            DiagnosticFinding::new(
                code::INSTALL_STATE_MISSING,
                DiagnosticSeverity::Info,
                DiagnosticComponent::Installation,
                "no package installation state was found",
            )
            .with_remedy("source builds may ignore this; package users can rerun the package-local installer")
            .with_evidence("path", redacted_path.clone()),
        );
        return InstallationReport {
            path: Some(redacted_path),
            state: "missing",
            installed_version: None,
            component_count: 0,
        };
    }

    match InstallState::load(&state_path) {
        Ok(state) => {
            let mut stale = state.ptymark_version != env!("CARGO_PKG_VERSION");
            if let Some(config_path) = selected_config_path {
                stale |= state.config_path != config_path;
            }
            if stale {
                findings.push(
                    DiagnosticFinding::new(
                        code::INSTALL_STATE_STALE,
                        DiagnosticSeverity::Warning,
                        DiagnosticComponent::Installation,
                        "installation state does not match the active binary or configuration",
                    )
                    .with_remedy("rerun the package-local installer or `ptymark install resolve`")
                    .with_evidence("path", redacted_path.clone()),
                );
            }
            InstallationReport {
                path: Some(redacted_path),
                state: if stale { "stale" } else { "valid" },
                installed_version: Some(state.ptymark_version),
                component_count: state.components.len(),
            }
        }
        Err(_error) => {
            findings.push(
                DiagnosticFinding::new(
                    code::INSTALL_STATE_STALE,
                    DiagnosticSeverity::Warning,
                    DiagnosticComponent::Installation,
                    "installation state is unreadable or invalid",
                )
                .with_remedy("rerun the package-local installer or `ptymark install resolve`")
                .with_evidence("path", redacted_path.clone())
                .with_evidence("error", DiagnosticEvidence::omitted()),
            );
            InstallationReport {
                path: Some(redacted_path),
                state: "invalid",
                installed_version: None,
                component_count: 0,
            }
        }
    }
}

fn inspect_engines(
    config: &Config,
    strict: bool,
    external_bypassed: bool,
    redactor: &Redactor,
    findings: &mut Vec<DiagnosticFinding>,
) -> (Vec<EngineReport>, PresenterReport) {
    let mut engines = Vec::new();
    let mermaid_external = config.engines.mermaid.backend.is_external() && !external_bypassed;
    let math_external = config.engines.math.backend.is_external() && !external_bypassed;

    engines.push(inspect_engine(
        "mermaid",
        config.engines.mermaid.backend.as_str(),
        &config.engines.mermaid.path,
        mermaid_external,
        strict,
        redactor,
        findings,
    ));
    engines.push(inspect_engine(
        "math",
        config.engines.math.backend.as_str(),
        &config.engines.math.path,
        math_external,
        strict,
        redactor,
        findings,
    ));

    let presenter_required = mermaid_external || math_external;
    let presenter = if presenter_required {
        let configured = redactor.public_path(&config.engines.presenter.path);
        match resolve_executable(&config.engines.presenter.path) {
            Ok(path) => PresenterReport {
                required: true,
                backend: "chafa-symbols",
                state: "ready",
                configured_path: Some(configured),
                resolved_path: Some(redactor.public_path(&path)),
            },
            Err(_error) => {
                findings.push(
                    DiagnosticFinding::new(
                        code::PRESENTER_UNSUPPORTED,
                        if strict {
                            DiagnosticSeverity::Error
                        } else {
                            DiagnosticSeverity::Warning
                        },
                        DiagnosticComponent::Presenter,
                        "the configured presenter is unavailable",
                    )
                    .with_remedy(
                        "select built-in preview/source or install the configured presenter",
                    )
                    .with_evidence("path", configured.clone())
                    .with_evidence("error", DiagnosticEvidence::omitted()),
                );
                PresenterReport {
                    required: true,
                    backend: "chafa-symbols",
                    state: "missing",
                    configured_path: Some(configured),
                    resolved_path: None,
                }
            }
        }
    } else {
        PresenterReport {
            required: false,
            backend: "not-required",
            state: "inactive",
            configured_path: None,
            resolved_path: None,
        }
    };
    (engines, presenter)
}

#[allow(clippy::too_many_arguments)]
fn inspect_engine(
    role: &'static str,
    backend: &str,
    configured_path: &Path,
    external: bool,
    strict: bool,
    redactor: &Redactor,
    findings: &mut Vec<DiagnosticFinding>,
) -> EngineReport {
    if !external {
        return EngineReport {
            role,
            backend: backend.to_owned(),
            origin: "built-in-or-bypassed",
            state: "built-in-or-bypassed",
            browser_state: None,
            configured_path: None,
            resolved_path: None,
        };
    }
    let configured = redactor.public_path(configured_path);
    match resolve_executable(configured_path) {
        Ok(path) => {
            let mut origin = if configured_path.is_absolute() {
                "explicit"
            } else {
                "path-search"
            };
            let mut state = "ready";
            let mut browser_state = None;
            if let Some(inspection) = inspect_managed_alias(&path) {
                origin = "managed-bundle";
                match inspection {
                    Ok(inspection) => {
                        if !inspection.complete {
                            state = "incompatible";
                            findings.push(
                                DiagnosticFinding::new(
                                    code::ENGINE_INCOMPATIBLE,
                                    if strict {
                                        DiagnosticSeverity::Error
                                    } else {
                                        DiagnosticSeverity::Warning
                                    },
                                    DiagnosticComponent::Engine,
                                    format!(
                                        "the managed {role} bundle is incomplete or incompatible"
                                    ),
                                )
                                .with_remedy("rerun the package-local managed renderer installer")
                                .with_evidence(
                                    "manifest",
                                    redactor.public_path(&inspection.manifest_path),
                                ),
                            );
                        }
                        if role == "mermaid" {
                            browser_state = Some(match inspection.browser_available {
                                Some(true) => "ready",
                                Some(false) => {
                                    findings.push(
                                        DiagnosticFinding::new(
                                            code::BROWSER_UNAVAILABLE,
                                            if strict {
                                                DiagnosticSeverity::Error
                                            } else {
                                                DiagnosticSeverity::Warning
                                            },
                                            DiagnosticComponent::Browser,
                                            "the managed Mermaid browser executable is unavailable",
                                        )
                                        .with_remedy(
                                            "rerun the managed bundle installer or select preview/source",
                                        ),
                                    );
                                    "missing"
                                }
                                None => "auto-or-unset",
                            });
                        }
                    }
                    Err(_error) => {
                        state = "incompatible";
                        findings.push(
                            DiagnosticFinding::new(
                                code::ENGINE_INCOMPATIBLE,
                                if strict {
                                    DiagnosticSeverity::Error
                                } else {
                                    DiagnosticSeverity::Warning
                                },
                                DiagnosticComponent::Engine,
                                format!("the managed {role} bundle manifest is invalid"),
                            )
                            .with_remedy("rerun the package-local managed renderer installer")
                            .with_evidence("error", DiagnosticEvidence::omitted()),
                        );
                    }
                }
            } else if role == "mermaid" {
                browser_state = Some("unknown-no-probe");
            }
            EngineReport {
                role,
                backend: backend.to_owned(),
                origin,
                state,
                browser_state,
                configured_path: Some(configured),
                resolved_path: Some(redactor.public_path(&path)),
            }
        }
        Err(_error) => {
            findings.push(
                DiagnosticFinding::new(
                    code::ENGINE_MISSING,
                    if strict {
                        DiagnosticSeverity::Error
                    } else {
                        DiagnosticSeverity::Warning
                    },
                    DiagnosticComponent::Engine,
                    format!("the configured {role} engine is unavailable"),
                )
                .with_remedy(format!(
                    "select preview/source for {role} or install the configured executable"
                ))
                .with_evidence("path", configured.clone())
                .with_evidence("error", DiagnosticEvidence::omitted()),
            );
            EngineReport {
                role,
                backend: backend.to_owned(),
                origin: if configured_path.is_absolute() {
                    "explicit"
                } else {
                    "path-search"
                },
                state: "missing",
                browser_state: if role == "mermaid" {
                    Some("unavailable")
                } else {
                    None
                },
                configured_path: Some(configured),
                resolved_path: None,
            }
        }
    }
}

fn inspect_terminal(findings: &mut Vec<DiagnosticFinding>) -> TerminalReport {
    let stdin_terminal = io::stdin().is_terminal();
    let stdout_terminal = io::stdout().is_terminal();
    if !(stdin_terminal && stdout_terminal) {
        findings.push(
            DiagnosticFinding::new(
                code::TERMINAL_REDIRECTED,
                DiagnosticSeverity::Info,
                DiagnosticComponent::Terminal,
                "stdin or stdout is redirected for this doctor invocation",
            )
            .with_remedy("use `preview` or `run -- COMMAND` for redirected streams; use native `-- COMMAND` for an interactive session"),
        );
    }
    let dimensions = crossterm::terminal::size().ok();
    let host = if cfg!(windows) {
        "conpty"
    } else if cfg!(unix) {
        "pty"
    } else {
        "unsupported"
    };
    if host == "unsupported" {
        findings.push(
            DiagnosticFinding::new(
                code::HOST_UNAVAILABLE,
                DiagnosticSeverity::Error,
                DiagnosticComponent::Host,
                "the current target has no native PTY/ConPTY host",
            )
            .with_remedy("use a supported Linux, macOS, or Windows build"),
        );
    }
    let mut hints = Vec::new();
    for (name, hint) in [
        ("WEZTERM_PANE", "wezterm"),
        ("TMUX", "tmux"),
        ("SSH_CONNECTION", "ssh"),
        ("WSL_DISTRO_NAME", "wsl"),
        ("MSYSTEM", "msys2-or-git-bash"),
    ] {
        if env::var_os(name).is_some() {
            hints.push(hint);
        }
    }
    hints.sort_unstable();
    TerminalReport {
        stdin_terminal,
        stdout_terminal,
        columns: dimensions.map(|(columns, _)| columns),
        rows: dimensions.map(|(_, rows)| rows),
        host,
        transport_hints: hints,
    }
}

fn terminal_word(value: bool) -> &'static str {
    if value { "terminal" } else { "redirected" }
}

fn push_line(output: &mut String, line: String) {
    output.push_str(&line);
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::{DOCTOR_SCHEMA, DoctorReport, DoctorRequest};
    use crate::runtime::PipelineOptions;

    #[test]
    fn json_schema_and_status_are_stable() {
        let report = DoctorReport::collect(DoctorRequest::default());
        let json = report.json();
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(value["schema"], DOCTOR_SCHEMA);
        assert!(value.get("status").is_some());
        assert!(value["findings"].is_array());
        assert!(!json.contains("semantic source"));
        assert!(json.ends_with('\n'));
    }

    #[test]
    fn session_modes_are_reported_without_starting_engines() {
        let request = DoctorRequest {
            pipeline: PipelineOptions {
                safe: true,
                private: true,
                ..PipelineOptions::default()
            },
            ..DoctorRequest::default()
        };
        let report = DoctorReport::collect(request);
        assert_eq!(report.session.mode, "safe");
        assert!(report.session.private);
        assert!(!report.session.cache_enabled);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "mode.safe")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "mode.private")
        );
    }

    #[test]
    fn support_report_is_atomic_and_refuses_overwrite() {
        let root = tempfile::tempdir().expect("temp root");
        let path = root.path().join("report.json");
        let report = DoctorReport::collect(DoctorRequest::default());
        report.write_support_report(&path).expect("write report");
        let source = std::fs::read_to_string(&path).expect("read report");
        assert!(source.contains(DOCTOR_SCHEMA));
        let error = report
            .write_support_report(&path)
            .expect_err("overwrite must fail");
        assert!(error.contains("already exists"));
    }
}
