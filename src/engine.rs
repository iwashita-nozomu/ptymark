use crate::config::{EnginesConfig, MathEngine, MermaidEngine};
use crate::diagnostics::code;
use crate::limits::{
    EXTERNAL_ATTEMPT_TIMEOUT, MAX_MATH_ARGUMENT_BYTES, MAX_PRESENTATION_BYTES,
    MAX_RENDER_ARTIFACT_BYTES, MAX_RENDERER_DIAGNOSTIC_BYTES, MAX_SVG_NODES, PROCESS_POLL_INTERVAL,
};
use crate::model::{BlockKind, SemanticBlock};
use crate::platform;
use crate::render::{
    PreviewRenderer, RenderArtifact, RenderCancellation, RenderContext, RenderError, Renderer,
    SourceRenderer,
};
use roxmltree::{Document, ParsingOptions};
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
pub(crate) const ENGINE_TIMEOUT: Duration = EXTERNAL_ATTEMPT_TIMEOUT;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineCheck {
    pub role: &'static str,
    pub backend: &'static str,
    pub configured_path: Option<PathBuf>,
    pub resolved_path: Option<PathBuf>,
}

impl EngineCheck {
    pub fn display_line(&self) -> String {
        match (&self.configured_path, &self.resolved_path) {
            (Some(configured), Some(resolved)) => format!(
                "{}\t{}\t{}\t{}",
                self.role,
                self.backend,
                configured.display(),
                resolved.display()
            ),
            _ => format!("{}\t{}\tbuilt-in", self.role, self.backend),
        }
    }
}

#[derive(Clone, Debug)]
enum EngineChoice {
    Preview,
    Source,
    MermaidCli(PathBuf),
    MathjaxCli(PathBuf),
}

pub struct ConfiguredRenderer {
    mermaid: EngineChoice,
    math: EngineChoice,
    presenter: PathBuf,
    timeout: Duration,
    cancellation: RenderCancellation,
    id: String,
}

impl ConfiguredRenderer {
    pub fn new(config: &EnginesConfig) -> Self {
        Self::with_cancellation(config, RenderCancellation::default())
    }

    pub fn with_cancellation(config: &EnginesConfig, cancellation: RenderCancellation) -> Self {
        let mermaid = match config.mermaid.backend {
            MermaidEngine::Preview => EngineChoice::Preview,
            MermaidEngine::Source => EngineChoice::Source,
            MermaidEngine::MermaidCli => EngineChoice::MermaidCli(config.mermaid.path.clone()),
        };
        let math = match config.math.backend {
            MathEngine::Preview => EngineChoice::Preview,
            MathEngine::Source => EngineChoice::Source,
            MathEngine::MathjaxCli => EngineChoice::MathjaxCli(config.math.path.clone()),
        };
        let id = format!(
            "configured-v1;mermaid={}:{};math={}:{};presenter={}",
            config.mermaid.backend.as_str(),
            config.mermaid.path.display(),
            config.math.backend.as_str(),
            config.math.path.display(),
            config.presenter.path.display()
        );
        Self {
            mermaid,
            math,
            presenter: config.presenter.path.clone(),
            timeout: ENGINE_TIMEOUT,
            cancellation,
            id,
        }
    }

    fn render_choice(
        &self,
        choice: &EngineChoice,
        block: &SemanticBlock,
        context: RenderContext,
    ) -> Result<RenderArtifact, RenderError> {
        match choice {
            EngineChoice::Preview => PreviewRenderer.render(block, context),
            EngineChoice::Source => SourceRenderer.render(block, context),
            EngineChoice::MermaidCli(path) => {
                let deadline = AttemptDeadline::new(self.timeout);
                let svg = render_mermaid_svg(path, block.body(), deadline, &self.cancellation)?;
                let bytes =
                    present_svg(&self.presenter, &svg, context, deadline, &self.cancellation)?;
                Ok(RenderArtifact::new(bytes))
            }
            EngineChoice::MathjaxCli(path) => {
                let deadline = AttemptDeadline::new(self.timeout);
                let svg = render_math_svg(path, block.body(), deadline, &self.cancellation)?;
                let bytes =
                    present_svg(&self.presenter, &svg, context, deadline, &self.cancellation)?;
                Ok(RenderArtifact::new(bytes))
            }
        }
    }
}

impl Renderer for ConfiguredRenderer {
    fn id(&self) -> &str {
        &self.id
    }

    fn render(
        &mut self,
        block: &SemanticBlock,
        context: RenderContext,
    ) -> Result<RenderArtifact, RenderError> {
        match block.kind() {
            BlockKind::Mermaid => self.render_choice(&self.mermaid, block, context),
            BlockKind::Math => self.render_choice(&self.math, block, context),
        }
    }
}

pub fn check_configured_engines(config: &EnginesConfig) -> Result<Vec<EngineCheck>, RenderError> {
    let mut checks = Vec::new();
    let mut requires_presenter = false;

    match config.mermaid.backend {
        MermaidEngine::Preview | MermaidEngine::Source => checks.push(EngineCheck {
            role: "mermaid",
            backend: config.mermaid.backend.as_str(),
            configured_path: None,
            resolved_path: None,
        }),
        MermaidEngine::MermaidCli => {
            let resolved = resolve_executable(&config.mermaid.path)?;
            checks.push(EngineCheck {
                role: "mermaid",
                backend: config.mermaid.backend.as_str(),
                configured_path: Some(config.mermaid.path.clone()),
                resolved_path: Some(resolved),
            });
            requires_presenter = true;
        }
    }

    match config.math.backend {
        MathEngine::Preview | MathEngine::Source => checks.push(EngineCheck {
            role: "math",
            backend: config.math.backend.as_str(),
            configured_path: None,
            resolved_path: None,
        }),
        MathEngine::MathjaxCli => {
            let resolved = resolve_executable(&config.math.path)?;
            checks.push(EngineCheck {
                role: "math",
                backend: config.math.backend.as_str(),
                configured_path: Some(config.math.path.clone()),
                resolved_path: Some(resolved),
            });
            requires_presenter = true;
        }
    }

    if requires_presenter {
        let resolved = resolve_executable(&config.presenter.path)?;
        checks.push(EngineCheck {
            role: "presenter",
            backend: "chafa-symbols",
            configured_path: Some(config.presenter.path.clone()),
            resolved_path: Some(resolved),
        });
    }

    Ok(checks)
}

pub fn resolve_executable(path: &Path) -> Result<PathBuf, RenderError> {
    platform::resolve_executable(path)
        .map_err(|message| RenderError::coded(code::ENGINE_MISSING, message))
}

fn render_mermaid_svg(
    program: &Path,
    body: &[u8],
    deadline: AttemptDeadline,
    cancellation: &RenderCancellation,
) -> Result<Vec<u8>, RenderError> {
    std::str::from_utf8(body)
        .map_err(|error| RenderError::new(format!("Mermaid input is not valid UTF-8: {error}")))?;
    let scratch = renderer_temp_dir()?;
    let output_path = scratch.path().join("diagram.svg");
    let arguments = vec![
        OsString::from("--input"),
        OsString::from("-"),
        OsString::from("--output"),
        output_path.clone().into_os_string(),
    ];
    run_process_with_deadline(
        program,
        &arguments,
        Some(body),
        MAX_RENDERER_DIAGNOSTIC_BYTES,
        deadline,
        cancellation,
    )?;
    let svg = read_file_capped(&output_path, MAX_RENDER_ARTIFACT_BYTES)?;
    validate_svg(&svg, "Mermaid CLI")?;
    Ok(svg)
}

fn render_math_svg(
    program: &Path,
    body: &[u8],
    deadline: AttemptDeadline,
    cancellation: &RenderCancellation,
) -> Result<Vec<u8>, RenderError> {
    let math = std::str::from_utf8(body)
        .map_err(|error| RenderError::new(format!("math input is not valid UTF-8: {error}")))?
        .trim();
    if math.is_empty() {
        return Err(RenderError::new("math input is empty"));
    }
    if math.len() > MAX_MATH_ARGUMENT_BYTES {
        return Err(RenderError::new(format!(
            "math input exceeds the {MAX_MATH_ARGUMENT_BYTES} byte mathjax-cli argument limit"
        )));
    }
    if math.as_bytes().contains(&0) {
        return Err(RenderError::new("math input contains a NUL byte"));
    }

    let arguments = vec![OsString::from(math)];
    let svg = run_process_with_deadline(
        program,
        &arguments,
        None,
        MAX_RENDER_ARTIFACT_BYTES,
        deadline,
        cancellation,
    )?;
    validate_svg(&svg, "MathJax CLI")?;
    Ok(svg)
}

fn present_svg(
    program: &Path,
    svg: &[u8],
    context: RenderContext,
    deadline: AttemptDeadline,
    cancellation: &RenderCancellation,
) -> Result<Vec<u8>, RenderError> {
    let scratch = renderer_temp_dir()?;
    let input_path = scratch.path().join("artifact.svg");
    fs::write(&input_path, svg).map_err(|error| {
        RenderError::new(format!(
            "cannot write temporary SVG `{}`: {error}",
            input_path.display()
        ))
    })?;

    let colors = if context.color { "full" } else { "none" };
    let arguments = vec![
        OsString::from("--format"),
        OsString::from("symbols"),
        OsString::from("--colors"),
        OsString::from(colors),
        OsString::from("--size"),
        OsString::from(format!("{}x", context.columns)),
        input_path.into_os_string(),
    ];
    let bytes = run_process_with_deadline(
        program,
        &arguments,
        None,
        MAX_PRESENTATION_BYTES,
        deadline,
        cancellation,
    )?;
    if bytes.is_empty() {
        return Err(RenderError::coded(
            code::PRESENTATION_FALLBACK,
            "Chafa presenter produced no display bytes",
        ));
    }
    Ok(bytes)
}

fn renderer_temp_dir() -> Result<TempDir, RenderError> {
    tempfile::Builder::new()
        .prefix("ptymark-render-")
        .tempdir()
        .map_err(|error| {
            RenderError::new(format!(
                "cannot create renderer temporary directory: {error}"
            ))
        })
}

fn validate_svg(bytes: &[u8], engine: &str) -> Result<(), RenderError> {
    if bytes.is_empty() {
        return Err(invalid_artifact(format!(
            "{engine} produced an empty SVG artifact"
        )));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|error| invalid_artifact(format!("{engine} output is not UTF-8 SVG: {error}")))?;
    let document = Document::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_SVG_NODES,
            entity_resolver: None,
        },
    )
    .map_err(|error| invalid_artifact(format!("{engine} output is malformed SVG XML: {error}")))?;
    let root = document.root_element();
    if root.tag_name().name() != "svg" {
        return Err(invalid_artifact(format!(
            "{engine} output root is `{}`, not `svg`",
            root.tag_name().name()
        )));
    }
    if root.tag_name().namespace() != Some(SVG_NAMESPACE) {
        return Err(invalid_artifact(format!(
            "{engine} output does not use the SVG namespace"
        )));
    }
    Ok(())
}

fn invalid_artifact(message: impl Into<String>) -> RenderError {
    RenderError::coded(code::RENDER_INVALID_ARTIFACT, message)
}

#[derive(Debug)]
struct CappedRead {
    bytes: Vec<u8>,
    overflowed: bool,
}

fn read_capped(mut reader: impl Read, limit: usize) -> io::Result<CappedRead> {
    let mut bytes = Vec::with_capacity(limit.min(8192));
    let mut overflowed = false;
    let mut chunk = [0_u8; 8192];

    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        let retained = remaining.min(count);
        bytes.extend_from_slice(&chunk[..retained]);
        overflowed |= retained < count;
    }

    Ok(CappedRead { bytes, overflowed })
}

fn read_file_capped(path: &Path, limit: usize) -> Result<Vec<u8>, RenderError> {
    let file = fs::File::open(path).map_err(|error| {
        RenderError::new(format!(
            "renderer did not create `{}`: {error}",
            path.display()
        ))
    })?;
    let mut bytes = Vec::with_capacity(limit.min(8192));
    file.take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            RenderError::new(format!(
                "cannot read renderer artifact `{}`: {error}",
                path.display()
            ))
        })?;
    if bytes.len() > limit {
        return Err(RenderError::coded(
            code::RENDER_OUTPUT_LIMIT,
            format!("renderer artifact exceeded {limit} bytes"),
        ));
    }
    Ok(bytes)
}

#[derive(Clone, Copy, Debug)]
struct AttemptDeadline {
    expires_at: Instant,
}

impl AttemptDeadline {
    fn new(timeout: Duration) -> Self {
        Self {
            expires_at: Instant::now() + timeout,
        }
    }

    fn remaining(self) -> Option<Duration> {
        self.expires_at.checked_duration_since(Instant::now())
    }
}

#[cfg(test)]
fn run_process_with_timeout(
    program: &Path,
    arguments: &[OsString],
    input: Option<&[u8]>,
    stdout_limit: usize,
    timeout: Duration,
) -> Result<Vec<u8>, RenderError> {
    run_process_with_deadline(
        program,
        arguments,
        input,
        stdout_limit,
        AttemptDeadline::new(timeout),
        &RenderCancellation::default(),
    )
}

fn run_process_with_deadline(
    program: &Path,
    arguments: &[OsString],
    input: Option<&[u8]>,
    stdout_limit: usize,
    deadline: AttemptDeadline,
    cancellation: &RenderCancellation,
) -> Result<Vec<u8>, RenderError> {
    let timeout = deadline.remaining().ok_or_else(|| {
        RenderError::coded(
            code::RENDER_TIMEOUT,
            format!(
                "renderer `{}` had no remaining attempt time",
                program_label(program)
            ),
        )
    })?;
    if cancellation.is_cancelled() {
        return Err(RenderError::coded(
            code::RENDER_OUTPUT_LIMIT,
            "the pending terminal-output limit cancelled the render attempt",
        ));
    }
    let mut command = Command::new(program);
    command
        .args(arguments)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let mut child = command.spawn().map_err(|error| {
        RenderError::coded(
            code::ENGINE_MISSING,
            format!(
                "cannot start renderer `{}`: {error}",
                program_label(program)
            ),
        )
    })?;

    let stdin_writer = if let Some(input) = input {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| RenderError::new("renderer stdin is unavailable"))?;
        let input = input.to_vec();
        Some(thread::spawn(move || {
            let result = stdin.write_all(&input);
            drop(stdin);
            result
        }))
    } else {
        None
    };

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| RenderError::new("renderer stdout is unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| RenderError::new("renderer stderr is unavailable"))?;
    let overflowed = Arc::new(AtomicBool::new(false));
    let stdout_overflowed = Arc::clone(&overflowed);
    let stdout_reader =
        thread::spawn(move || read_capped_until_limit(stdout, stdout_limit, &stdout_overflowed));
    let stderr_reader = thread::spawn(move || read_capped(stderr, MAX_RENDERER_DIAGNOSTIC_BYTES));

    let outcome = wait_with_limits(&mut child, timeout, &overflowed, cancellation);
    if outcome.is_err() {
        terminate_child(&mut child);
    }

    let writer_result = stdin_writer.map(|writer| {
        writer
            .join()
            .map_err(|_| RenderError::new("renderer stdin writer panicked"))?
            .map_err(|error| RenderError::new(format!("renderer input failed: {error}")))
    });
    let stdout = stdout_reader
        .join()
        .map_err(|_| RenderError::new("renderer stdout reader panicked"))?
        .map_err(|error| RenderError::new(format!("renderer output read failed: {error}")))?;
    let _stderr = stderr_reader
        .join()
        .map_err(|_| RenderError::new("renderer stderr reader panicked"))?
        .map_err(|error| RenderError::new(format!("renderer stderr read failed: {error}")))?;
    let outcome = outcome.map_err(|error| {
        RenderError::coded(
            code::RENDER_PROCESS_EXIT,
            format!("renderer `{}` wait failed: {error}", program_label(program)),
        )
    })?;

    match outcome {
        ProcessOutcome::TimedOut => {
            return Err(RenderError::coded(
                code::RENDER_TIMEOUT,
                format!(
                    "renderer `{}` exceeded {} ms timeout",
                    program_label(program),
                    timeout.as_millis()
                ),
            ));
        }
        ProcessOutcome::OutputLimit => {
            return Err(RenderError::coded(
                code::RENDER_OUTPUT_LIMIT,
                format!(
                    "renderer `{}` output exceeded {stdout_limit} bytes",
                    program_label(program)
                ),
            ));
        }
        ProcessOutcome::Cancelled => {
            return Err(RenderError::coded(
                code::RENDER_OUTPUT_LIMIT,
                format!(
                    "renderer `{}` was cancelled after pending terminal output reached its bound",
                    program_label(program)
                ),
            ));
        }
        ProcessOutcome::Exited(status) => {
            if stdout.overflowed || overflowed.load(Ordering::Acquire) {
                return Err(RenderError::coded(
                    code::RENDER_OUTPUT_LIMIT,
                    format!(
                        "renderer `{}` output exceeded {stdout_limit} bytes",
                        program_label(program)
                    ),
                ));
            }
            if !status.success() {
                return Err(RenderError::coded(
                    code::RENDER_PROCESS_EXIT,
                    format!(
                        "renderer `{}` exited with {status}; diagnostic output was redacted",
                        program_label(program)
                    ),
                ));
            }
            if let Some(result) = writer_result {
                result.map_err(|_| {
                    RenderError::coded(
                        code::RENDER_PROCESS_EXIT,
                        format!(
                            "renderer `{}` did not accept its complete input",
                            program_label(program)
                        ),
                    )
                })?;
            }
        }
    }

    Ok(stdout.bytes)
}

fn read_capped_until_limit(
    mut reader: impl Read,
    limit: usize,
    overflowed: &AtomicBool,
) -> io::Result<CappedRead> {
    let mut bytes = Vec::with_capacity(limit.min(8192));
    let mut chunk = [0_u8; 8192];
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        let retained = remaining.min(count);
        bytes.extend_from_slice(&chunk[..retained]);
        if retained < count || bytes.len() == limit {
            let mut probe = [0_u8; 1];
            if retained < count || reader.read(&mut probe)? != 0 {
                overflowed.store(true, Ordering::Release);
                return Ok(CappedRead {
                    bytes,
                    overflowed: true,
                });
            }
            break;
        }
    }
    Ok(CappedRead {
        bytes,
        overflowed: false,
    })
}

#[derive(Clone, Copy, Debug)]
enum ProcessOutcome {
    Exited(ExitStatus),
    TimedOut,
    OutputLimit,
    Cancelled,
}

fn wait_with_limits(
    child: &mut Child,
    timeout: Duration,
    overflowed: &AtomicBool,
    cancellation: &RenderCancellation,
) -> io::Result<ProcessOutcome> {
    let started = Instant::now();
    loop {
        if cancellation.is_cancelled() {
            terminate_child(child);
            return Ok(ProcessOutcome::Cancelled);
        }
        if overflowed.load(Ordering::Acquire) {
            terminate_child(child);
            return Ok(ProcessOutcome::OutputLimit);
        }
        if let Some(status) = child.try_wait()? {
            return Ok(ProcessOutcome::Exited(status));
        }
        if started.elapsed() >= timeout {
            terminate_child(child);
            return Ok(ProcessOutcome::TimedOut);
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    {
        let process_group = format!("-{}", child.id());
        let _ = Command::new("/bin/kill")
            .arg("-KILL")
            .arg("--")
            .arg(process_group)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn program_label(program: &Path) -> String {
    program.file_name().map_or_else(
        || "renderer".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::{
        ConfiguredRenderer, check_configured_engines, resolve_executable, run_process_with_timeout,
        validate_svg,
    };
    use crate::config::{Config, MathEngine, MermaidEngine};
    use crate::diagnostics::code;
    use crate::model::{BlockKind, SemanticBlock};
    use crate::render::{RenderContext, Renderer};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::Duration;
    use tempfile::TempDir;

    fn temp_root() -> TempDir {
        tempfile::Builder::new()
            .prefix("ptymark-engine-test-")
            .tempdir()
            .expect("temp root")
    }

    fn executable(path: &Path, source: &str) {
        fs::write(path, source).expect("write executable");
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod");
    }

    #[test]
    fn built_in_engine_check_requires_no_external_programs() {
        let checks = check_configured_engines(&Config::default().engines).expect("check");
        assert_eq!(checks.len(), 2);
        assert!(checks.iter().all(|check| check.resolved_path.is_none()));
    }

    #[test]
    fn executable_resolution_accepts_an_absolute_executable() {
        let root = temp_root();
        let path = root.path().join("engine");
        executable(&path, "#!/bin/sh\nexit 0\n");
        let resolved = resolve_executable(&path).expect("resolve");
        assert!(resolved.is_absolute());
    }

    #[test]
    fn mermaid_cli_is_presented_through_chafa() {
        let root = temp_root();
        let mmdc = root.path().join("mmdc");
        let chafa = root.path().join("chafa");
        executable(
            &mmdc,
            "#!/bin/sh\nout=''\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    --output) out=$2; shift 2 ;;\n    *) shift ;;\n  esac\ndone\ncat >/dev/null\nprintf '<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>' >\"$out\"\n",
        );
        executable(&chafa, "#!/bin/sh\nprintf 'terminal diagram\\n'\n");

        let mut config = Config::default();
        config.engines.mermaid.backend = MermaidEngine::MermaidCli;
        config.engines.mermaid.path = mmdc;
        config.engines.presenter.path = chafa;
        let mut renderer = ConfiguredRenderer::new(&config.engines);
        let block = SemanticBlock::new(
            BlockKind::Mermaid,
            b"```mermaid\nA --> B\n```\n".to_vec(),
            b"A --> B\n".to_vec(),
        );
        let artifact = renderer
            .render(&block, RenderContext::default())
            .expect("render");
        assert_eq!(artifact.bytes, b"terminal diagram\n");
    }

    #[test]
    fn mathjax_cli_is_presented_through_chafa() {
        let root = temp_root();
        let tex2svg = root.path().join("tex2svg");
        let chafa = root.path().join("chafa");
        executable(
            &tex2svg,
            "#!/bin/sh\nprintf '<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>\\n'\n",
        );
        executable(&chafa, "#!/bin/sh\nprintf 'terminal math\\n'\n");

        let mut config = Config::default();
        config.engines.math.backend = MathEngine::MathjaxCli;
        config.engines.math.path = tex2svg;
        config.engines.presenter.path = chafa;
        let mut renderer = ConfiguredRenderer::new(&config.engines);
        let block = SemanticBlock::new(
            BlockKind::Math,
            b"$$\nE = mc^2\n$$\n".to_vec(),
            b"E = mc^2\n".to_vec(),
        );
        let artifact = renderer
            .render(&block, RenderContext::default())
            .expect("render");
        assert_eq!(artifact.bytes, b"terminal math\n");
    }

    #[test]
    fn structurally_rejects_text_containing_an_svg_substring() {
        let error = validate_svg(
            b"not an artifact <svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
            "test",
        )
        .expect_err("unrelated leading text must fail");
        assert_eq!(error.code(), code::RENDER_INVALID_ARTIFACT);
    }

    #[test]
    fn structurally_rejects_a_non_svg_root() {
        let error = validate_svg(b"<html><svg/></html>", "test").expect_err("wrong root must fail");
        assert_eq!(error.code(), code::RENDER_INVALID_ARTIFACT);
    }

    #[test]
    fn renderer_timeout_has_a_stable_code_and_redacted_message() {
        let root = temp_root();
        let renderer = root.path().join("slow-renderer");
        executable(&renderer, "#!/bin/sh\nsleep 5\n");
        let error = run_process_with_timeout(&renderer, &[], None, 1024, Duration::from_millis(50))
            .expect_err("renderer must time out");
        assert_eq!(error.code(), code::RENDER_TIMEOUT);
        assert!(
            !error
                .to_string()
                .contains(root.path().to_string_lossy().as_ref())
        );
    }

    #[test]
    fn renderer_output_limit_stops_the_process() {
        let root = temp_root();
        let renderer = root.path().join("noisy-renderer");
        executable(
            &renderer,
            "#!/bin/sh\nwhile :; do printf '0123456789abcdef'; done\n",
        );
        let error = run_process_with_timeout(&renderer, &[], None, 128, Duration::from_secs(2))
            .expect_err("renderer output must be bounded");
        assert_eq!(error.code(), code::RENDER_OUTPUT_LIMIT);
    }

    #[test]
    fn renderer_stderr_is_not_copied_into_public_errors() {
        let root = temp_root();
        let renderer = root.path().join("failing-renderer");
        executable(
            &renderer,
            "#!/bin/sh\nprintf 'PRIVATE SEMANTIC SOURCE token-123\\n' >&2\nexit 7\n",
        );
        let error = run_process_with_timeout(&renderer, &[], None, 1024, Duration::from_secs(2))
            .expect_err("renderer must fail");
        assert_eq!(error.code(), code::RENDER_PROCESS_EXIT);
        assert!(!error.to_string().contains("PRIVATE SEMANTIC SOURCE"));
        assert!(!error.to_string().contains("token-123"));
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::run_process_with_timeout;
    use crate::diagnostics::code;
    use std::ffi::OsString;
    use std::path::Path;
    use std::time::Duration;

    #[test]
    fn powershell_renderer_timeout_is_bounded() {
        let arguments = vec![
            OsString::from("-NoLogo"),
            OsString::from("-NoProfile"),
            OsString::from("-NonInteractive"),
            OsString::from("-Command"),
            OsString::from("Start-Sleep -Seconds 5"),
        ];
        let error = run_process_with_timeout(
            Path::new("powershell.exe"),
            &arguments,
            None,
            1024,
            Duration::from_millis(100),
        )
        .expect_err("renderer must time out");
        assert_eq!(error.code(), code::RENDER_TIMEOUT);
    }

    #[test]
    fn powershell_renderer_output_is_bounded() {
        let arguments = vec![
            OsString::from("-NoLogo"),
            OsString::from("-NoProfile"),
            OsString::from("-NonInteractive"),
            OsString::from("-Command"),
            OsString::from("while ($true) { [Console]::Out.Write('0123456789abcdef') }"),
        ];
        let error = run_process_with_timeout(
            Path::new("powershell.exe"),
            &arguments,
            None,
            128,
            Duration::from_secs(3),
        )
        .expect_err("renderer output must be bounded");
        assert_eq!(error.code(), code::RENDER_OUTPUT_LIMIT);
    }
}
