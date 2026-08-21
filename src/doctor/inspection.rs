use super::{InstallationReport, TerminalReport};
use crate::config::Config;
use crate::diagnostics::{
    DiagnosticComponent, DiagnosticEvidence, DiagnosticFinding, DiagnosticSeverity, Redactor, code,
};
use crate::install::{InstallState, default_install_state_path};
use std::env;
use std::io::{self, IsTerminal};
use std::path::Path;

pub(super) fn load_configuration(
    explicit: Option<&Path>,
    profile: Option<&str>,
    selected_path: Option<&Path>,
    findings: &mut Vec<DiagnosticFinding>,
) -> (Config, &'static str, Option<u32>) {
    let result = match explicit {
        Some(path) => Config::load_exact_profile(path, profile),
        None => Config::load_profile(None, profile),
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

pub(super) fn inspect_installation(
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

pub(super) fn inspect_terminal(findings: &mut Vec<DiagnosticFinding>) -> TerminalReport {
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
