use crate::config::{Config, RenderMode};
use crate::diagnostics::{
    DiagnosticComponent, DiagnosticEvidence, DiagnosticFinding, DiagnosticSeverity,
    DiagnosticStatus, Redactor, code, json_string,
};
use crate::engine::resolve_executable;
use crate::install::{InstallState, default_install_state_path};
use crate::managed_launcher::inspect_managed_alias;
use crate::runtime::PipelineOptions;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

pub const DOCTOR_SCHEMA: &str = "ptymark.doctor.v1";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DoctorRequest {
    pub config_path: Option<PathBuf>,
    pub pipeline: PipelineOptions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PtymarkReport {
    pub version: String,
    pub target_os: &'static str,
    pub target_arch: &'static str,
    pub config_schema: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationReport {
    pub selection: &'static str,
    pub path: Option<DiagnosticEvidence>,
    pub state: &'static str,
    pub schema_version: Option<u32>,
    pub strict: bool,
    pub rendering_mode: &'static str,
    pub cache_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationReport {
    pub path: Option<DiagnosticEvidence>,
    pub state: &'static str,
    pub installed_version: Option<String>,
    pub component_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionReport {
    pub mode: &'static str,
    pub private: bool,
    pub strict: bool,
    pub cache_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalReport {
    pub stdin_terminal: bool,
    pub stdout_terminal: bool,
    pub columns: Option<u16>,
    pub rows: Option<u16>,
    pub host: &'static str,
    pub transport_hints: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineReport {
    pub role: &'static str,
    pub backend: String,
    pub origin: &'static str,
    pub state: &'static str,
    pub browser_state: Option<&'static str>,
    pub configured_path: Option<DiagnosticEvidence>,
    pub resolved_path: Option<DiagnosticEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresenterReport {
    pub required: bool,
    pub backend: &'static str,
    pub state: &'static str,
    pub configured_path: Option<DiagnosticEvidence>,
    pub resolved_path: Option<DiagnosticEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecentRuntimeReport {
    pub state: &'static str,
    pub finding_code: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
        let mut output = String::new();
        output.push_str("{\n");
        json_field(&mut output, 1, "schema", json_string(self.schema), true);
        json_field(
            &mut output,
            1,
            "status",
            json_string(self.status.as_str()),
            true,
        );
        output.push_str("  \"ptymark\": {\n");
        json_field(
            &mut output,
            2,
            "version",
            json_string(&self.ptymark.version),
            true,
        );
        json_field(
            &mut output,
            2,
            "target_os",
            json_string(self.ptymark.target_os),
            true,
        );
        json_field(
            &mut output,
            2,
            "target_arch",
            json_string(self.ptymark.target_arch),
            true,
        );
        json_field(
            &mut output,
            2,
            "config_schema",
            self.ptymark.config_schema.to_string(),
            false,
        );
        output.push_str("  },\n");

        output.push_str("  \"configuration\": {\n");
        json_field(
            &mut output,
            2,
            "selection",
            json_string(self.configuration.selection),
            true,
        );
        json_field(
            &mut output,
            2,
            "path",
            evidence_json(self.configuration.path.as_ref()),
            true,
        );
        json_field(
            &mut output,
            2,
            "state",
            json_string(self.configuration.state),
            true,
        );
        json_field(
            &mut output,
            2,
            "schema_version",
            option_number(self.configuration.schema_version),
            true,
        );
        json_field(
            &mut output,
            2,
            "strict",
            self.configuration.strict.to_string(),
            true,
        );
        json_field(
            &mut output,
            2,
            "rendering_mode",
            json_string(self.configuration.rendering_mode),
            true,
        );
        json_field(
            &mut output,
            2,
            "cache_enabled",
            self.configuration.cache_enabled.to_string(),
            false,
        );
        output.push_str("  },\n");

        output.push_str("  \"installation\": {\n");
        json_field(
            &mut output,
            2,
            "path",
            evidence_json(self.installation.path.as_ref()),
            true,
        );
        json_field(
            &mut output,
            2,
            "state",
            json_string(self.installation.state),
            true,
        );
        json_field(
            &mut output,
            2,
            "installed_version",
            self.installation
                .installed_version
                .as_deref()
                .map_or_else(|| "null".to_owned(), json_string),
            true,
        );
        json_field(
            &mut output,
            2,
            "component_count",
            self.installation.component_count.to_string(),
            false,
        );
        output.push_str("  },\n");

        output.push_str("  \"session\": {\n");
        json_field(&mut output, 2, "mode", json_string(self.session.mode), true);
        json_field(
            &mut output,
            2,
            "private",
            self.session.private.to_string(),
            true,
        );
        json_field(
            &mut output,
            2,
            "strict",
            self.session.strict.to_string(),
            true,
        );
        json_field(
            &mut output,
            2,
            "cache_enabled",
            self.session.cache_enabled.to_string(),
            false,
        );
        output.push_str("  },\n");

        output.push_str("  \"terminal\": {\n");
        json_field(
            &mut output,
            2,
            "stdin_terminal",
            self.terminal.stdin_terminal.to_string(),
            true,
        );
        json_field(
            &mut output,
            2,
            "stdout_terminal",
            self.terminal.stdout_terminal.to_string(),
            true,
        );
        json_field(
            &mut output,
            2,
            "columns",
            option_number(self.terminal.columns),
            true,
        );
        json_field(
            &mut output,
            2,
            "rows",
            option_number(self.terminal.rows),
            true,
        );
        json_field(
            &mut output,
            2,
            "host",
            json_string(self.terminal.host),
            true,
        );
        let hints = self
            .terminal
            .transport_hints
            .iter()
            .map(|hint| json_string(hint))
            .collect::<Vec<_>>()
            .join(", ");
        json_field(
            &mut output,
            2,
            "transport_hints",
            format!("[{hints}]"),
            false,
        );
        output.push_str("  },\n");

        output.push_str("  \"engines\": [\n");
        for (index, engine) in self.engines.iter().enumerate() {
            output.push_str("    {\n");
            json_field(&mut output, 3, "role", json_string(engine.role), true);
            json_field(
                &mut output,
                3,
                "backend",
                json_string(&engine.backend),
                true,
            );
            json_field(&mut output, 3, "origin", json_string(engine.origin), true);
            json_field(&mut output, 3, "state", json_string(engine.state), true);
            json_field(
                &mut output,
                3,
                "browser_state",
                engine
                    .browser_state
                    .map_or_else(|| "null".to_owned(), json_string),
                true,
            );
            json_field(
                &mut output,
                3,
                "configured_path",
                evidence_json(engine.configured_path.as_ref()),
                true,
            );
            json_field(
                &mut output,
                3,
                "resolved_path",
                evidence_json(engine.resolved_path.as_ref()),
                false,
            );
            output.push_str(if index + 1 == self.engines.len() {
                "    }\n"
            } else {
                "    },\n"
            });
        }
        output.push_str("  ],\n");

        output.push_str("  \"presenter\": {\n");
        json_field(
            &mut output,
            2,
            "required",
            self.presenter.required.to_string(),
            true,
        );
        json_field(
            &mut output,
            2,
            "backend",
            json_string(self.presenter.backend),
            true,
        );
        json_field(
            &mut output,
            2,
            "state",
            json_string(self.presenter.state),
            true,
        );
        json_field(
            &mut output,
            2,
            "configured_path",
            evidence_json(self.presenter.configured_path.as_ref()),
            true,
        );
        json_field(
            &mut output,
            2,
            "resolved_path",
            evidence_json(self.presenter.resolved_path.as_ref()),
            false,
        );
        output.push_str("  },\n");

        output.push_str("  \"recent_runtime\": {\n");
        json_field(
            &mut output,
            2,
            "state",
            json_string(self.recent_runtime.state),
            true,
        );
        json_field(
            &mut output,
            2,
            "finding_code",
            self.recent_runtime
                .finding_code
                .as_deref()
                .map_or_else(|| "null".to_owned(), json_string),
            false,
        );
        output.push_str("  },\n");

        output.push_str("  \"findings\": [\n");
        for (index, finding) in self.findings.iter().enumerate() {
            output.push_str("    {\n");
            json_field(&mut output, 3, "code", json_string(&finding.code), true);
            json_field(
                &mut output,
                3,
                "severity",
                json_string(finding.severity.as_str()),
                true,
            );
            json_field(
                &mut output,
                3,
                "component",
                json_string(finding.component.as_str()),
                true,
            );
            json_field(
                &mut output,
                3,
                "summary",
                json_string(&finding.summary),
                true,
            );
            json_field(
                &mut output,
                3,
                "remedy",
                finding
                    .remedy
                    .as_deref()
                    .map_or_else(|| "null".to_owned(), json_string),
                true,
            );
            output.push_str("      \"evidence\": {");
            if finding.evidence.is_empty() {
                output.push_str("}\n");
            } else {
                output.push('\n');
                for (evidence_index, (key, evidence)) in finding.evidence.iter().enumerate() {
                    output.push_str(&format!(
                        "        {}: {{\"value\": {}, \"redacted\": {}}}{}\n",
                        json_string(key),
                        json_string(&evidence.value),
                        evidence.redacted,
                        if evidence_index + 1 == finding.evidence.len() {
                            ""
                        } else {
                            ","
                        }
                    ));
                }
                output.push_str("      }\n");
            }
            output.push_str(if index + 1 == self.findings.len() {
                "    }\n"
            } else {
                "    },\n"
            });
        }
        output.push_str("  ],\n");

        output.push_str("  \"redaction\": {\n");
        json_field(
            &mut output,
            2,
            "public_safe_default",
            self.redaction.public_safe_default.to_string(),
            true,
        );
        json_field(
            &mut output,
            2,
            "semantic_source",
            json_string(self.redaction.semantic_source),
            true,
        );
        json_field(
            &mut output,
            2,
            "child_environment",
            json_string(self.redaction.child_environment),
            true,
        );
        json_field(
            &mut output,
            2,
            "renderer_stderr",
            json_string(self.redaction.renderer_stderr),
            true,
        );
        json_field(
            &mut output,
            2,
            "home_paths",
            json_string(self.redaction.home_paths),
            false,
        );
        output.push_str("  }\n");
        output.push_str("}\n");
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
            .filter(|parent| !parent.as_os_str().is_empty());
        if let Some(parent) = parent
            && !parent.is_dir()
        {
            return Err(format!(
                "support report directory `{}` does not exist",
                parent.display()
            ));
        }
        let file_name = path
            .file_name()
            .ok_or_else(|| "support report path has no file name".to_owned())?;
        let temporary = path.with_file_name(format!(
            ".{}.tmp-{}",
            file_name.to_string_lossy(),
            std::process::id()
        ));
        let result = write_private_file(&temporary, self.json().as_bytes())
            .and_then(|()| fs::rename(&temporary, path));
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(format!(
                "cannot write support report `{}`: {error}",
                path.display()
            ));
        }
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
    let mut presenter_required = false;
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
    presenter_required |= mermaid_external || math_external;

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

fn json_field(output: &mut String, indent: usize, key: &str, value: String, comma: bool) {
    output.push_str(&"  ".repeat(indent));
    output.push_str(&json_string(key));
    output.push_str(": ");
    output.push_str(&value);
    if comma {
        output.push(',');
    }
    output.push('\n');
}

fn option_number<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}

fn evidence_json(evidence: Option<&DiagnosticEvidence>) -> String {
    evidence.map_or_else(
        || "null".to_owned(),
        |evidence| {
            format!(
                "{{\"value\": {}, \"redacted\": {}}}",
                json_string(&evidence.value),
                evidence.redacted
            )
        },
    )
}

fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::{DOCTOR_SCHEMA, DoctorReport, DoctorRequest};
    use crate::runtime::PipelineOptions;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("ptymark-doctor-{label}-{nonce}.json"))
    }

    #[test]
    fn json_schema_and_status_are_stable() {
        let report = DoctorReport::collect(DoctorRequest::default());
        let json = report.json();
        assert!(json.contains(&format!("\"schema\": \"{DOCTOR_SCHEMA}\"")));
        assert!(json.contains("\"status\":"));
        assert!(json.contains("\"findings\": ["));
        assert!(!json.contains("semantic source"));
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
        let path = temp_path("report");
        let report = DoctorReport::collect(DoctorRequest::default());
        report.write_support_report(&path).expect("write report");
        let source = fs::read_to_string(&path).expect("read report");
        assert!(source.contains(DOCTOR_SCHEMA));
        let error = report
            .write_support_report(&path)
            .expect_err("overwrite must fail");
        assert!(error.contains("already exists"));
        let _ = fs::remove_file(path);
    }
}
