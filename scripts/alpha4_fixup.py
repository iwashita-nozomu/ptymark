"""Apply the bounded, reviewed alpha.4 source fixes before compiler validation."""

from __future__ import annotations

from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    target = Path(path)
    source = target.read_text(encoding="utf-8")
    if old in source:
        target.write_text(source.replace(old, new, 1), encoding="utf-8")
        return
    if new and new in source:
        return
    raise RuntimeError(f"{label}: neither source nor replacement fragment was found in {path}")


def main() -> None:
    replace_once(
        "src/doctor.rs",
        """pub struct DoctorRequest {
    pub config_path: Option<PathBuf>,
    pub pipeline: PipelineOptions,
}""",
        """pub struct DoctorRequest {
    pub config_path: Option<PathBuf>,
    pub profile: Option<String>,
    pub pipeline: PipelineOptions,
}""",
        "doctor request profile",
    )
    replace_once(
        "src/doctor.rs",
        """        let (config, config_state, schema_version) = load_configuration(
            request.config_path.as_deref(),
            selected_config_path.as_deref(),
            &mut findings,
        );""",
        """        let (config, config_state, schema_version) = load_configuration(
            request.config_path.as_deref(),
            request.profile.as_deref(),
            selected_config_path.as_deref(),
            &mut findings,
        );""",
        "doctor profile resolution call",
    )
    replace_once(
        "src/doctor.rs",
        """fn load_configuration(
    explicit: Option<&Path>,
    selected_path: Option<&Path>,
    findings: &mut Vec<DiagnosticFinding>,
) -> (Config, &'static str, Option<u32>) {
    let result = match explicit {
        Some(path) => Config::load_exact(path),
        None => Config::load(None),
    };""",
        """fn load_configuration(
    explicit: Option<&Path>,
    profile: Option<&str>,
    selected_path: Option<&Path>,
    findings: &mut Vec<DiagnosticFinding>,
) -> (Config, &'static str, Option<u32>) {
    let result = match explicit {
        Some(path) => Config::load_exact_profile(path, profile),
        None => Config::load_profile(None, profile),
    };""",
        "doctor profile-aware loader",
    )

    replace_once(
        "src/install.rs",
        """use crate::config::{
    Config, EngineProvider, EngineSelection, MathEngine, MermaidEngine, PresenterProvider,
    PresenterSelection, UserConfig,
};""",
        """use crate::config::{
    Config, EngineProvider, EngineSelection, PresenterProvider, PresenterSelection, UserConfig,
};""",
        "unused engine imports",
    )

    replace_once(
        "src/config.rs",
        """        let user = UserConfig::load_exact(path)?;
        let state = load_matching_install_state(path);
        user.resolve(selected_profile, state.as_ref())""",
        """        let user = UserConfig::load_exact(path)?;
        let state = load_matching_install_state(path, &user);
        user.resolve(selected_profile, state.as_ref())""",
        "config state identity call",
    )
    replace_once(
        "src/config.rs",
        """fn load_matching_install_state(config_path: &Path) -> Option<InstallState> {
    let state_path = default_install_state_path().ok()?;
    let state = InstallState::load(&state_path).ok()?;
    (state.config_path == config_path).then_some(state)
}""",
        """fn load_matching_install_state(
    config_path: &Path,
    user: &UserConfig,
) -> Option<InstallState> {
    let state_path = default_install_state_path().ok()?;
    let state = InstallState::load(&state_path).ok()?;
    state.matches_user_config(config_path, user).then_some(state)
}""",
        "config state digest matching",
    )
    replace_once(
        "src/config.rs",
        "self.rendering.columns.max(1).min(MAX_FALLBACK_COLUMNS)",
        "self.rendering.columns.clamp(1, MAX_FALLBACK_COLUMNS)",
        "legacy fallback column clamp",
    )

    replace_once(
        "src/pipeline.rs",
        "pub const MAX_PENDING_OUTPUT_BYTES: usize = 1024 * 1024;",
        "pub const MAX_PENDING_OUTPUT_BYTES: usize = crate::limits::MAX_PENDING_TERMINAL_BYTES;",
        "pipeline internal pending-output limit",
    )

    replace_once(
        "src/native_session.rs",
        """use crate::command::ChildCommand;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, size as terminal_size};""",
        """use crate::command::ChildCommand;
#[cfg(windows)]
use crate::limits::CONPTY_OUTPUT_DRAIN_GRACE;
use crate::limits::{DEFAULT_PTY_ROWS, RESIZE_POLL_INTERVAL};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, size as terminal_size};""",
        "native session internal limits import",
    )
    replace_once(
        "src/native_session.rs",
        """use std::thread::{self, JoinHandle};
use std::time::Duration;

const DEFAULT_ROWS: u16 = 24;
const RESIZE_POLL_INTERVAL: Duration = Duration::from_millis(80);
#[cfg(windows)]
const CONPTY_OUTPUT_DRAIN_GRACE: Duration = Duration::from_millis(100);
""",
        """use std::thread::{self, JoinHandle};
""",
        "native session duplicate constants",
    )
    replace_once(
        "src/native_session.rs",
        "unwrap_or(DEFAULT_ROWS)",
        "unwrap_or(DEFAULT_PTY_ROWS)",
        "native session fallback rows",
    )

    replace_once(
        "src/routing.rs",
        """                RenderContext {
                    columns: 123,
                    color: false,
                    theme_fingerprint: 7,
                },""",
        """                RenderContext {
                    columns: 123,
                    color: false,
                    plain: false,
                    theme_fingerprint: 7,
                },""",
        "routing render context",
    )
    replace_once(
        "tests/routing_contract.rs",
        """            RenderContext {
                columns: 96,
                color: false,
                theme_fingerprint: 11,
            },""",
        """            RenderContext {
                columns: 96,
                color: false,
                plain: false,
                theme_fingerprint: 11,
            },""",
        "routing contract render context",
    )
    replace_once(
        "tests/pipeline_contract.rs",
        """        RenderContext {
            columns: 80,
            color: false,
            theme_fingerprint: 0,
        },""",
        """        RenderContext {
            columns: 80,
            color: false,
            plain: false,
            theme_fingerprint: 0,
        },""",
        "pipeline contract render context",
    )

    replace_once(
        "src/cli.rs",
        "Some(value) if value.is_empty() => return Err(format!(\"`{option}` cannot be empty\")),",
        "Some(\"\") => return Err(format!(\"`{option}` cannot be empty\")),",
        "engine preference empty value",
    )
    replace_once(
        "src/cli.rs",
        "Some(value) if value.is_empty() => return Err(\"`--presenter` cannot be empty\".to_owned()),",
        "Some(\"\") => return Err(\"`--presenter` cannot be empty\".to_owned()),",
        "presenter preference empty value",
    )


if __name__ == "__main__":
    main()
