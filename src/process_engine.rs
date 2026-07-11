use crate::artifact::{ArtifactFormat, RenderArtifact};
use crate::engine::{EngineDescriptor, ExecutionModel, RenderEngine, RenderRequest};
use crate::model::BlockKind;
use crate::renderer::RenderError;
use crate::ui::LayoutSensitivity;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProgramResolution {
    #[default]
    AbsoluteOnly,
    PathSearch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessEngineConfig {
    pub id: String,
    pub version: String,
    pub supported_kinds: Vec<BlockKind>,
    pub formats: Vec<ArtifactFormat>,
    pub layout_sensitivity: LayoutSensitivity,
    pub execution_model: ExecutionModel,
    pub program: PathBuf,
    pub program_resolution: ProgramResolution,
    pub arguments: Vec<OsString>,
    pub timeout: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub working_directory: Option<PathBuf>,
    pub environment: BTreeMap<OsString, OsString>,
    pub inherit_environment: Vec<OsString>,
}

impl ProcessEngineConfig {
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        program: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            supported_kinds: vec![BlockKind::Math, BlockKind::Mermaid],
            formats: vec![ArtifactFormat::TerminalText],
            layout_sensitivity: LayoutSensitivity::Independent,
            execution_model: ExecutionModel::OneShotProcess,
            program: program.into(),
            program_resolution: ProgramResolution::AbsoluteOnly,
            arguments: Vec::new(),
            timeout: Duration::from_secs(5),
            max_stdout_bytes: 4 * 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
            working_directory: None,
            environment: BTreeMap::new(),
            inherit_environment: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), RenderError> {
        if self.id.trim().is_empty() {
            return Err(RenderError::new("process engine ID cannot be empty"));
        }
        if self.version.trim().is_empty() {
            return Err(RenderError::new(format!(
                "process engine `{}` version cannot be empty",
                self.id
            )));
        }
        if self.supported_kinds.is_empty() {
            return Err(RenderError::new(format!(
                "process engine `{}` must support at least one semantic kind",
                self.id
            )));
        }
        if self.formats.is_empty() {
            return Err(RenderError::new(format!(
                "process engine `{}` must expose at least one artifact format",
                self.id
            )));
        }
        if self.program.as_os_str().is_empty() {
            return Err(RenderError::new(format!(
                "process engine `{}` program cannot be empty",
                self.id
            )));
        }
        if self.program_resolution == ProgramResolution::AbsoluteOnly && !self.program.is_absolute()
        {
            return Err(RenderError::new(format!(
                "process engine `{}` requires an absolute program path: {}",
                self.id,
                self.program.display()
            )));
        }
        if let Some(directory) = &self.working_directory
            && !directory.is_absolute()
        {
            return Err(RenderError::new(format!(
                "process engine `{}` requires an absolute working directory: {}",
                self.id,
                directory.display()
            )));
        }
        if self.timeout.is_zero() {
            return Err(RenderError::new(format!(
                "process engine `{}` timeout must be greater than zero",
                self.id
            )));
        }
        if self.max_stdout_bytes == 0 || self.max_stderr_bytes == 0 {
            return Err(RenderError::new(format!(
                "process engine `{}` stdout/stderr limits must be greater than zero",
                self.id
            )));
        }
        for key in self
            .environment
            .keys()
            .chain(self.inherit_environment.iter())
        {
            let text = key.to_string_lossy();
            if text.is_empty() || text.contains('=') {
                return Err(RenderError::new(format!(
                    "process engine `{}` contains an invalid environment key",
                    self.id
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ProcessEngine {
    config: ProcessEngineConfig,
    descriptor: EngineDescriptor,
}

impl ProcessEngine {
    pub fn new(config: ProcessEngineConfig) -> Result<Self, RenderError> {
        config.validate()?;
        let descriptor = EngineDescriptor::new(
            config.id.clone(),
            config.version.clone(),
            config.supported_kinds.clone(),
            config.formats.clone(),
            config.layout_sensitivity,
            config.execution_model,
        );
        Ok(Self { config, descriptor })
    }

    pub fn config(&self) -> &ProcessEngineConfig {
        &self.config
    }

    fn run(&self, request: &RenderRequest<'_>) -> Result<Vec<u8>, RenderError> {
        let mut command = Command::new(&self.config.program);
        command.args(&self.config.arguments).env_clear();

        for key in &self.config.inherit_environment {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        for (key, value) in &self.config.environment {
            command.env(key, value);
        }

        command
            .env("PTYMARK_RENDERER_PROTOCOL", "stdio-v1")
            .env("PTYMARK_RENDERER_ID", &self.config.id)
            .env("PTYMARK_BLOCK_KIND", request.block.kind().as_str())
            .env(
                "PTYMARK_SOURCE_BYTES",
                request.block.source().len().to_string(),
            )
            .env(
                "PTYMARK_COLOR",
                if request.context.color { "1" } else { "0" },
            )
            .env(
                "PTYMARK_TERMINAL_WIDTH",
                request
                    .context
                    .terminal_width
                    .map(|width| width.to_string())
                    .unwrap_or_default(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(directory) = &self.config.working_directory {
            command.current_dir(directory);
        }

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        let mut child = command.spawn().map_err(|error| {
            RenderError::new(format!(
                "renderer `{}` could not start `{}`: {error}",
                self.config.id,
                self.config.program.display()
            ))
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| RenderError::new("renderer stdin is unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| RenderError::new("renderer stdout is unavailable"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| RenderError::new("renderer stderr is unavailable"))?;

        let body = request.block.body().to_vec();
        let stdin_writer = thread::spawn(move || {
            let mut stdin = stdin;
            let result = stdin.write_all(&body);
            drop(stdin);
            result
        });
        let stdout_limit = self.config.max_stdout_bytes;
        let stderr_limit = self.config.max_stderr_bytes;
        let stdout_reader = thread::spawn(move || read_capped(stdout, stdout_limit));
        let stderr_reader = thread::spawn(move || read_capped(stderr, stderr_limit));

        let status = wait_with_timeout(&mut child, self.config.timeout).map_err(|error| {
            terminate_child(&mut child);
            RenderError::new(format!(
                "renderer `{}` wait failed: {error}",
                self.config.id
            ))
        })?;

        if status.is_none() {
            let _ = stdin_writer.join();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(RenderError::new(format!(
                "renderer `{}` exceeded timeout of {} ms",
                self.config.id,
                self.config.timeout.as_millis()
            )));
        }

        stdin_writer
            .join()
            .map_err(|_| RenderError::new("renderer stdin writer panicked"))?
            .map_err(|error| RenderError::new(format!("renderer input failed: {error}")))?;
        let stdout_result = stdout_reader
            .join()
            .map_err(|_| RenderError::new("renderer stdout reader panicked"))?
            .map_err(|error| RenderError::new(format!("renderer output read failed: {error}")))?;
        let stderr_result = stderr_reader
            .join()
            .map_err(|_| RenderError::new("renderer stderr reader panicked"))?
            .map_err(|error| {
                RenderError::new(format!("renderer diagnostic read failed: {error}"))
            })?;

        if stdout_result.overflowed {
            return Err(RenderError::new(format!(
                "renderer `{}` stdout exceeded {} bytes",
                self.config.id, self.config.max_stdout_bytes
            )));
        }
        if stderr_result.overflowed {
            return Err(RenderError::new(format!(
                "renderer `{}` stderr exceeded {} bytes",
                self.config.id, self.config.max_stderr_bytes
            )));
        }

        let status = status.expect("status checked above");
        if !status.success() {
            let diagnostic = String::from_utf8_lossy(&stderr_result.bytes);
            let diagnostic = diagnostic.trim();
            let suffix = if diagnostic.is_empty() {
                String::new()
            } else {
                format!(": {diagnostic}")
            };
            return Err(RenderError::new(format!(
                "renderer `{}` exited with {status}{suffix}",
                self.config.id
            )));
        }
        if stdout_result.bytes.is_empty() {
            return Err(RenderError::new(format!(
                "renderer `{}` returned an empty artifact",
                self.config.id
            )));
        }

        Ok(stdout_result.bytes)
    }
}

impl RenderEngine for ProcessEngine {
    fn descriptor(&self) -> &EngineDescriptor {
        &self.descriptor
    }

    fn render(&mut self, request: &RenderRequest<'_>) -> Result<RenderArtifact, RenderError> {
        if !self.descriptor.supports_kind(request.block.kind()) {
            return Err(RenderError::new(format!(
                "renderer `{}` does not support `{}`",
                self.config.id,
                request.block.kind()
            )));
        }
        if !self.descriptor.formats.contains(&request.preferred_format) {
            return Err(RenderError::new(format!(
                "renderer `{}` cannot produce `{}`",
                self.config.id,
                request.preferred_format.as_str()
            )));
        }
        Ok(RenderArtifact::new(
            request.preferred_format,
            self.run(request)?,
            self.descriptor.identity.clone(),
            request.block.kind(),
            self.descriptor.layout_sensitivity,
        ))
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
    use super::{ProcessEngine, ProcessEngineConfig, ProgramResolution};
    use crate::artifact::ArtifactFormat;
    use crate::engine::{ExecutionModel, RenderEngine, RenderRequest};
    use crate::model::{BlockKind, SemanticBlock};
    use crate::renderer::RenderContext;
    use crate::ui::LayoutSensitivity;
    use std::ffi::OsString;
    use std::time::Duration;

    fn block() -> SemanticBlock {
        SemanticBlock::new(
            BlockKind::Math,
            b"$$\nE = mc^2\n$$\n".to_vec(),
            b"E = mc^2\n".to_vec(),
        )
    }

    fn config(program: &str) -> ProcessEngineConfig {
        ProcessEngineConfig {
            id: "test/process".to_owned(),
            version: "1".to_owned(),
            supported_kinds: vec![BlockKind::Math],
            formats: vec![ArtifactFormat::TerminalText],
            layout_sensitivity: LayoutSensitivity::Independent,
            execution_model: ExecutionModel::OneShotProcess,
            program: program.into(),
            program_resolution: ProgramResolution::AbsoluteOnly,
            arguments: Vec::new(),
            timeout: Duration::from_secs(1),
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
            working_directory: None,
            environment: Default::default(),
            inherit_environment: Vec::new(),
        }
    }

    #[test]
    fn process_engine_uses_raw_stdio_without_a_shell() {
        let mut engine = ProcessEngine::new(config("/bin/cat")).expect("engine");
        let block = block();
        let artifact = engine
            .render(&RenderRequest {
                block: &block,
                context: &RenderContext::default(),
                preferred_format: ArtifactFormat::TerminalText,
            })
            .expect("render");
        assert_eq!(artifact.bytes, block.body());
    }

    #[test]
    fn process_engine_clears_unlisted_environment() {
        let mut config = config("/bin/sh");
        config.arguments = vec![
            OsString::from("-c"),
            OsString::from("test -z \"$HOME\" && printf clean"),
        ];
        let mut engine = ProcessEngine::new(config).expect("engine");
        let block = block();
        let artifact = engine
            .render(&RenderRequest {
                block: &block,
                context: &RenderContext::default(),
                preferred_format: ArtifactFormat::TerminalText,
            })
            .expect("render");
        assert_eq!(artifact.bytes, b"clean");
    }

    #[test]
    fn process_engine_inherits_only_allowlisted_environment() {
        let mut config = config("/bin/sh");
        config.arguments = vec![
            OsString::from("-c"),
            OsString::from("test -n \"$PATH\" && printf allowed"),
        ];
        config.inherit_environment = vec![OsString::from("PATH")];
        let mut engine = ProcessEngine::new(config).expect("engine");
        let block = block();
        let artifact = engine
            .render(&RenderRequest {
                block: &block,
                context: &RenderContext::default(),
                preferred_format: ArtifactFormat::TerminalText,
            })
            .expect("render");
        assert_eq!(artifact.bytes, b"allowed");
    }

    #[test]
    fn process_engine_enforces_timeout() {
        let mut config = config("/bin/sh");
        config.arguments = vec![OsString::from("-c"), OsString::from("sleep 1")];
        config.timeout = Duration::from_millis(20);
        let mut engine = ProcessEngine::new(config).expect("engine");
        let block = block();
        let error = engine
            .render(&RenderRequest {
                block: &block,
                context: &RenderContext::default(),
                preferred_format: ArtifactFormat::TerminalText,
            })
            .expect_err("timeout");
        assert!(error.to_string().contains("timeout"));
    }

    #[test]
    fn path_search_must_be_explicit() {
        let mut config = config("node");
        assert!(ProcessEngine::new(config.clone()).is_err());
        config.program_resolution = ProgramResolution::PathSearch;
        assert!(ProcessEngine::new(config).is_ok());
    }
}
