mod inspection;
mod output;

use self::inspection::{inspect_installation, inspect_terminal, load_configuration};
use crate::config::{Config, RenderMode};
use crate::diagnostics::{
    DiagnosticComponent, DiagnosticEvidence, DiagnosticFinding, DiagnosticSeverity,
    DiagnosticStatus, Redactor, code,
};
use crate::engine::resolve_executable;
use crate::managed_launcher::{
    BROWSER_RUNTIME_LAUNCH_FAILED, BROWSER_RUNTIME_LIBRARIES_MISSING, BROWSER_RUNTIME_TIMEOUT,
    ManagedRuntimeStatus, inspect_managed_alias, probe_managed_alias,
};
use crate::runtime::PipelineOptions;
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};

pub const DOCTOR_SCHEMA: &str = "ptymark.doctor.v1";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DoctorRequest {
    pub config_path: Option<PathBuf>,
    pub profile: Option<String>,
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
    pub runtime_state: Option<&'static str>,
    pub browser_state: Option<&'static str>,
    pub configured_path: Option<DiagnosticEvidence>,
    pub resolved_path: Option<DiagnosticEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PresenterReport {
    pub required: bool,
    pub backend: &'static str,
    pub state: &'static str,
    pub runtime_state: Option<&'static str>,
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
            request.profile.as_deref(),
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
}

fn inspect_engines(
    config: &Config,
    strict: bool,
    external_bypassed: bool,
    redactor: &Redactor,
    findings: &mut Vec<DiagnosticFinding>,
) -> (Vec<EngineReport>, PresenterReport) {
    let mermaid_external = config.engines.mermaid.backend.is_external() && !external_bypassed;
    let math_external = config.engines.math.backend.is_external() && !external_bypassed;
    let engines = vec![
        inspect_engine(
            "mermaid",
            config.engines.mermaid.backend.as_str(),
            &config.engines.mermaid.path,
            mermaid_external,
            strict,
            redactor,
            findings,
        ),
        inspect_engine(
            "math",
            config.engines.math.backend.as_str(),
            &config.engines.math.path,
            math_external,
            strict,
            redactor,
            findings,
        ),
    ];
    let presenter = inspect_presenter(
        &config.engines.presenter.path,
        mermaid_external || math_external,
        strict,
        redactor,
        findings,
    );
    (engines, presenter)
}

fn inspect_presenter(
    configured_path: &Path,
    required: bool,
    strict: bool,
    redactor: &Redactor,
    findings: &mut Vec<DiagnosticFinding>,
) -> PresenterReport {
    if !required {
        return PresenterReport {
            required: false,
            backend: "not-required",
            state: "inactive",
            runtime_state: None,
            configured_path: None,
            resolved_path: None,
        };
    }

    let configured = redactor.public_path(configured_path);
    match resolve_executable(configured_path) {
        Ok(path) => {
            let mut state = "ready";
            let mut runtime_state = None;
            if let Some(inspection) = inspect_managed_alias(&path) {
                match inspection {
                    Ok(inspection) if inspection.complete => {
                        runtime_state = Some(inspect_managed_runtime(
                            "presenter",
                            &path,
                            strict,
                            findings,
                        ));
                    }
                    Ok(_) | Err(_) => {
                        state = "incompatible";
                        runtime_state = Some("incompatible");
                        findings.push(
                            DiagnosticFinding::new(
                                code::PRESENTER_UNSUPPORTED,
                                severity(strict),
                                DiagnosticComponent::Presenter,
                                "the managed presenter bundle is incomplete or incompatible",
                            )
                            .with_remedy("rerun the package-local managed renderer installer")
                            .with_evidence("error", DiagnosticEvidence::omitted()),
                        );
                    }
                }
            }
            PresenterReport {
                required: true,
                backend: "chafa-symbols",
                state,
                runtime_state,
                configured_path: Some(configured),
                resolved_path: Some(redactor.public_path(&path)),
            }
        }
        Err(_error) => {
            findings.push(
                DiagnosticFinding::new(
                    code::PRESENTER_UNSUPPORTED,
                    severity(strict),
                    DiagnosticComponent::Presenter,
                    "the configured presenter is unavailable",
                )
                .with_remedy("select built-in preview/source or install the configured presenter")
                .with_evidence("path", configured.clone())
                .with_evidence("error", DiagnosticEvidence::omitted()),
            );
            PresenterReport {
                required: true,
                backend: "chafa-symbols",
                state: "missing",
                runtime_state: None,
                configured_path: Some(configured),
                resolved_path: None,
            }
        }
    }
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
            runtime_state: None,
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
            let mut runtime_state = None;
            let mut browser_state = if role == "mermaid" {
                Some("unknown-no-probe")
            } else {
                None
            };

            if let Some(inspection) = inspect_managed_alias(&path) {
                origin = "managed-bundle";
                match inspection {
                    Ok(inspection) => {
                        if role == "mermaid" {
                            browser_state = Some(match inspection.browser_available {
                                Some(true) => "present",
                                Some(false) => "missing",
                                None => "auto-or-unset",
                            });
                        }
                        if inspection.complete {
                            runtime_state =
                                Some(inspect_managed_runtime(role, &path, strict, findings));
                        } else {
                            state = "incompatible";
                            runtime_state = Some("incompatible");
                            findings.push(
                                DiagnosticFinding::new(
                                    code::ENGINE_INCOMPATIBLE,
                                    severity(strict),
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
                            if role == "mermaid" && inspection.browser_available == Some(false) {
                                findings.push(
                                    DiagnosticFinding::new(
                                        code::BROWSER_UNAVAILABLE,
                                        severity(strict),
                                        DiagnosticComponent::Browser,
                                        "the managed Mermaid browser executable is unavailable",
                                    )
                                    .with_remedy(
                                        "rerun the managed bundle installer or select preview/source",
                                    ),
                                );
                            }
                        }
                    }
                    Err(_error) => {
                        state = "incompatible";
                        runtime_state = Some("incompatible");
                        findings.push(
                            DiagnosticFinding::new(
                                code::ENGINE_INCOMPATIBLE,
                                severity(strict),
                                DiagnosticComponent::Engine,
                                format!("the managed {role} bundle manifest is invalid"),
                            )
                            .with_remedy("rerun the package-local managed renderer installer")
                            .with_evidence("error", DiagnosticEvidence::omitted()),
                        );
                    }
                }
            }

            EngineReport {
                role,
                backend: backend.to_owned(),
                origin,
                state,
                runtime_state,
                browser_state,
                configured_path: Some(configured),
                resolved_path: Some(redactor.public_path(&path)),
            }
        }
        Err(_error) => {
            findings.push(
                DiagnosticFinding::new(
                    code::ENGINE_MISSING,
                    severity(strict),
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
                runtime_state: None,
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

fn inspect_managed_runtime(
    role: &'static str,
    path: &Path,
    strict: bool,
    findings: &mut Vec<DiagnosticFinding>,
) -> &'static str {
    match probe_managed_alias(path) {
        Some(Ok(status)) => {
            record_runtime_finding(role, &status, strict, findings);
            status.as_str()
        }
        Some(Err(_error)) => {
            findings.push(
                DiagnosticFinding::new(
                    if role == "presenter" {
                        code::PRESENTER_UNSUPPORTED
                    } else {
                        code::ENGINE_INCOMPATIBLE
                    },
                    severity(strict),
                    if role == "presenter" {
                        DiagnosticComponent::Presenter
                    } else {
                        DiagnosticComponent::Engine
                    },
                    format!("the managed {role} runtime probe could not start"),
                )
                .with_remedy("rerun the package-local managed renderer installer")
                .with_evidence("error", DiagnosticEvidence::omitted()),
            );
            "launch-failed"
        }
        None => "not-applicable",
    }
}

fn record_runtime_finding(
    role: &'static str,
    status: &ManagedRuntimeStatus,
    strict: bool,
    findings: &mut Vec<DiagnosticFinding>,
) {
    match status {
        ManagedRuntimeStatus::Ready => {}
        ManagedRuntimeStatus::MissingLibraries { libraries } => {
            let evidence = libraries.join(",");
            let duplicate = findings.iter().any(|finding| {
                finding.code == BROWSER_RUNTIME_LIBRARIES_MISSING
                    && finding
                        .evidence
                        .get("libraries")
                        .is_some_and(|value| value.value == evidence)
            });
            if !duplicate {
                findings.push(
                    DiagnosticFinding::new(
                        BROWSER_RUNTIME_LIBRARIES_MISSING,
                        severity(strict),
                        DiagnosticComponent::Browser,
                        "the managed Chromium runtime is missing host shared libraries",
                    )
                    .with_remedy(ubuntu_library_remedy(libraries))
                    .with_evidence("libraries", DiagnosticEvidence::visible(evidence)),
                );
            }
        }
        ManagedRuntimeStatus::TimedOut => findings.push(
            DiagnosticFinding::new(
                BROWSER_RUNTIME_TIMEOUT,
                severity(strict),
                DiagnosticComponent::Browser,
                format!("the managed {role} runtime probe exceeded its hard deadline"),
            )
            .with_remedy(
                "select preview/source, verify the browser can start headlessly, then rerun `ptymark doctor`",
            ),
        ),
        ManagedRuntimeStatus::LaunchFailed | ManagedRuntimeStatus::InvalidArtifact => {
            if role == "math" {
                findings.push(
                    DiagnosticFinding::new(
                        code::ENGINE_INCOMPATIBLE,
                        severity(strict),
                        DiagnosticComponent::Engine,
                        "the managed math runtime could not produce a valid SVG",
                    )
                    .with_remedy(
                        "rerun the package-local managed renderer installer or select preview/source",
                    ),
                );
            } else {
                findings.push(
                    DiagnosticFinding::new(
                        BROWSER_RUNTIME_LAUNCH_FAILED,
                        severity(strict),
                        DiagnosticComponent::Browser,
                        format!("the managed {role} browser path could not produce a valid sample"),
                    )
                    .with_remedy(
                        "install the reported host libraries, verify headless Chromium, then rerun `ptymark doctor`",
                    ),
                );
            }
        }
    }
}

fn ubuntu_library_remedy(libraries: &[String]) -> String {
    let needs_nspr = libraries.iter().any(|library| library == "libnspr4.so");
    let needs_nss = libraries.iter().any(|library| {
        matches!(
            library.as_str(),
            "libnss3.so" | "libnssutil3.so" | "libsmime3.so"
        )
    });
    let mut packages = Vec::new();
    if needs_nspr {
        packages.push("libnspr4");
    }
    if needs_nss {
        packages.push("libnss3");
    }
    if packages.is_empty() {
        return "on Ubuntu 22.04/24.04 or WSL, install the package providing each listed shared library, then rerun `ptymark doctor`".to_owned();
    }
    format!(
        "on Ubuntu 22.04/24.04 or WSL, run `sudo apt-get update && sudo apt-get install --yes {}`; install the package providing any additional listed library, then rerun `ptymark doctor`",
        packages.join(" ")
    )
}

const fn severity(strict: bool) -> DiagnosticSeverity {
    if strict {
        DiagnosticSeverity::Error
    } else {
        DiagnosticSeverity::Warning
    }
}

#[cfg(test)]
mod tests {
    use super::{DOCTOR_SCHEMA, DoctorReport, DoctorRequest, ubuntu_library_remedy};
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

    #[test]
    fn ubuntu_nss_and_nspr_remediation_is_stable() {
        let remedy = ubuntu_library_remedy(&[
            "libnss3.so".to_owned(),
            "libnspr4.so".to_owned(),
            "libsmime3.so".to_owned(),
        ]);
        assert!(remedy.contains("Ubuntu 22.04/24.04 or WSL"));
        assert!(remedy.contains("apt-get install --yes libnspr4 libnss3"));
        assert!(remedy.contains("ptymark doctor"));
    }
}
