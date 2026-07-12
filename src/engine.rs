use crate::config::{EnginesConfig, MathEngine, MermaidEngine};
use crate::model::{BlockKind, SemanticBlock};
use crate::render::{
    PreviewRenderer, RenderArtifact, RenderContext, RenderError, Renderer, SourceRenderer,
};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const ENGINE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_ARTIFACT_BYTES: usize = 8 * 1024 * 1024;
const MAX_DISPLAY_BYTES: usize = 8 * 1024 * 1024;
const MAX_DIAGNOSTIC_BYTES: usize = 64 * 1024;
const MAX_MATH_ARGUMENT_BYTES: usize = 32 * 1024;

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    id: String,
}

impl ConfiguredRenderer {
    pub fn new(config: &EnginesConfig) -> Self {
        let mermaid = match config.mermaid.backend {
            MermaidEngine::Preview => EngineChoice::Preview,
            MermaidEngine::Source => EngineChoice::Source,
            MermaidEngine::MermaidCli => {
                EngineChoice::MermaidCli(config.mermaid.path.clone())
            }
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
                let svg = render_mermaid_svg(path, block.body())?;
                let bytes = present_svg(&self.presenter, &svg, context)?;
                Ok(RenderArtifact::new(bytes))
            }
            EngineChoice::MathjaxCli(path) => {
                let svg = render_math_svg(path, block.body())?;
                let bytes = present_svg(&self.presenter, &svg, context)?;
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
    if path.is_absolute() {
        return executable_candidate(path).ok_or_else(|| {
            RenderError::new(format!(
                "configured executable `{}` does not exist or is not executable",
                path.display()
            ))
        });
    }

    if path.components().count() != 1 {
        return Err(RenderError::new(format!(
            "configured executable `{}` must be absolute or a bare name",
            path.display()
        )));
    }

    let search_path = env::var_os("PATH")
        .ok_or_else(|| RenderError::new("PATH is not set; use an absolute engine path"))?;
    for directory in env::split_paths(&search_path) {
        let candidate = directory.join(path);
        if let Some(candidate) = executable_candidate(&candidate) {
            return Ok(candidate);
        }
    }

    Err(RenderError::new(format!(
        "executable `{}` was not found in PATH; set its absolute path in the ptymark configuration",
        path.display()
    )))
}

fn executable_candidate(path: &Path) -> Option<PathBuf> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }

    Some(fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}

fn render_mermaid_svg(program: &Path, body: &[u8]) -> Result<Vec<u8>, RenderError> {
    std::str::from_utf8(body)
        .map_err(|error| RenderError::new(format!("Mermaid input is not valid UTF-8: {error}")))?;
    let scratch = ScratchDir::create()?;
    let output_path = scratch.path().join("diagram.svg");
    let arguments = vec![
        OsString::from("--input"),
        OsString::from("-"),
        OsString::from("--output"),
        output_path.clone().into_os_string(),
    ];
    run_process(program, &arguments, Some(body), MAX_DIAGNOSTIC_BYTES)?;
    let svg = read_file_capped(&output_path, MAX_ARTIFACT_BYTES)?;
    validate_svg(&svg, "Mermaid CLI")?;
    Ok(svg)
}

fn render_math_svg(program: &Path, body: &[u8]) -> Result<Vec<u8>, RenderError> {
    let math = std::str::from_utf8(body)
        .map_err(|error| RenderError::new(format!("math input is not valid UTF-8: {error}")))?
        .trim();
    if math.is_empty() {
        return Err(RenderError::new("math input is empty"));
    }
    if math.len() > MAX_MATH_ARGUMENT_BYTES {
        return Err(RenderError::new(format!(
            "math input exceeds the {} byte mathjax-cli argument limit",
            MAX_MATH_ARGUMENT_BYTES
        )));
    }
    if math.as_bytes().contains(&0) {
        return Err(RenderError::new("math input contains a NUL byte"));
    }

    let arguments = vec![OsString::from(math)];
    let svg = run_process(program, &arguments, None, MAX_ARTIFACT_BYTES)?;
    validate_svg(&svg, "MathJax CLI")?;
    Ok(svg)
}

fn present_svg(
    program: &Path,
    svg: &[u8],
    context: RenderContext,
) -> Result<Vec<u8>, RenderError> {
    let scratch = ScratchDir::create()?;
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
        OsString::from("--probe"),
        OsString::from("off"),
        OsString::from("--polite"),
        OsString::from("on"),
        OsString::from("--relative"),
        OsString::from("off"),
        OsString::from("--animate"),
        OsString::from("off"),
        OsString::from("--colors"),
        OsString::from(colors),
        OsString::from("--size"),
        OsString::from(format!("{}x", context.columns)),
        input_path.into_os_string(),
    ];
    let bytes = run_process(program, &arguments, None, MAX_DISPLAY_BYTES)?;
    if bytes.is_empty() {
        return Err(RenderError::new("Chafa presenter produced no display bytes"));
    }
    Ok(bytes)
}

fn validate_svg(bytes: &[u8], engine: &str) -> Result<(), RenderError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| RenderError::new(format!("{engine} output is not UTF-8 SVG: {error}")))?;
    if !text.contains("<svg") {
        return Err(RenderError::new(format!(
            "{engine} output does not contain an SVG element"
        )));
    }
    Ok(())
}

#[derive(Debug)]
struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    fn create() -> Result<Self, RenderError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for _ in 0..32 {
            let counter = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "ptymark-{}-{timestamp}-{counter}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(RenderError::new(format!(
                        "cannot create temporary renderer directory `{}`: {error}",
                        path.display()
                    )));
                }
            }
        }
        Err(RenderError::new(
            "cannot allocate a unique temporary renderer directory",
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
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
    let result = read_capped(file, limit).map_err(|error| {
        RenderError::new(format!(
            "cannot read renderer artifact `{}`: {error}",
            path.display()
        ))
    })?;
    if result.overflowed {
        return Err(RenderError::new(format!(
            "renderer artifact exceeded {limit} bytes"
        )));
    }
    Ok(result.bytes)
}

fn run_process(
    program: &Path,
    arguments: &[OsString],
    input: Option<&[u8]>,
    stdout_limit: usize,
) -> Result<Vec<u8>, RenderError> {
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
        RenderError::new(format!(
            "cannot start renderer `{}`: {error}",
            program.display()
        ))
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
    let stdout_reader = thread::spawn(move || read_capped(stdout, stdout_limit));
    let stderr_reader = thread::spawn(move || read_capped(stderr, MAX_DIAGNOSTIC_BYTES));

    let status = match wait_with_timeout(&mut child, ENGINE_TIMEOUT) {
        Ok(Some(status)) => status,
        Ok(None) => {
            return Err(RenderError::new(format!(
                "renderer `{}` exceeded {} ms timeout",
                program.display(),
                ENGINE_TIMEOUT.as_millis()
            )));
        }
        Err(error) => {
            terminate_child(&mut child);
            return Err(RenderError::new(format!(
                "renderer `{}` wait failed: {error}",
                program.display()
            )));
        }
    };

    if let Some(writer) = stdin_writer {
        writer
            .join()
            .map_err(|_| RenderError::new("renderer stdin writer panicked"))?
            .map_err(|error| RenderError::new(format!("renderer input failed: {error}")))?;
    }

    let stdout = stdout_reader
        .join()
        .map_err(|_| RenderError::new("renderer stdout reader panicked"))?
        .map_err(|error| RenderError::new(format!("renderer output read failed: {error}")))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| RenderError::new("renderer stderr reader panicked"))?
        .map_err(|error| RenderError::new(format!("renderer stderr read failed: {error}")))?;

    if stdout.overflowed {
        return Err(RenderError::new(format!(
            "renderer `{}` output exceeded {stdout_limit} bytes",
            program.display()
        )));
    }
    if !status.success() {
        let diagnostic = String::from_utf8_lossy(&stderr.bytes);
        let diagnostic = diagnostic.trim();
        let suffix = if diagnostic.is_empty() {
            String::new()
        } else {
            format!(": {diagnostic}")
        };
        return Err(RenderError::new(format!(
            "renderer `{}` exited with {status}{suffix}",
            program.display()
        )));
    }

    Ok(stdout.bytes)
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
    let _ = child.kill();
    let _ = child.wait();
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> io::Result<Option<ExitStatus>> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if started.elapsed() >= timeout {
            terminate_child(child);
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::{ConfiguredRenderer, check_configured_engines, resolve_executable};
    use crate::config::{Config, MathEngine, MermaidEngine};
    use crate::model::{BlockKind, SemanticBlock};
    use crate::render::{RenderContext, Renderer};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ptymark-engine-{label}-{nonce}"));
        fs::create_dir_all(&path).expect("temp root");
        path
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
        let root = temp_root("resolve");
        let path = root.join("engine");
        executable(&path, "#!/bin/sh\nexit 0\n");
        let resolved = resolve_executable(&path).expect("resolve");
        assert!(resolved.is_absolute());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mermaid_cli_is_presented_through_chafa() {
        let root = temp_root("mermaid");
        let mmdc = root.join("mmdc");
        let chafa = root.join("chafa");
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
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mathjax_cli_is_presented_through_chafa() {
        let root = temp_root("math");
        let tex2svg = root.join("tex2svg");
        let chafa = root.join("chafa");
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
        let _ = fs::remove_dir_all(root);
    }
}