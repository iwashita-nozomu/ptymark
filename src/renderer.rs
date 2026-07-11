use crate::model::SemanticBlock;
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const STDERR_LIMIT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderContext {
    pub color: bool,
    pub terminal_width: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderError {
    message: String,
}

impl RenderError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for RenderError {}

pub trait BlockRenderer: Send {
    fn render(
        &mut self,
        block: &SemanticBlock,
        context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError>;
}

impl<T: BlockRenderer + ?Sized> BlockRenderer for Box<T> {
    fn render(
        &mut self,
        block: &SemanticBlock,
        context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError> {
        (**self).render(block, context)
    }
}

#[derive(Debug, Default)]
pub struct PreviewRenderer;

impl BlockRenderer for PreviewRenderer {
    fn render(
        &mut self,
        block: &SemanticBlock,
        context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError> {
        let mut output = Vec::new();
        if context.color {
            output.extend_from_slice(b"\x1b[1;36m");
        }
        output.extend_from_slice(format!("┌─ ptymark {} preview ─\n", block.kind()).as_bytes());
        if context.color {
            output.extend_from_slice(b"\x1b[0m");
        }

        let body = String::from_utf8_lossy(block.body());
        if body.is_empty() {
            output.extend_from_slice("│ <empty>\n".as_bytes());
        } else {
            for line in body.split_inclusive('\n') {
                let line = line.strip_suffix('\n').unwrap_or(line);
                let line = line.strip_suffix('\r').unwrap_or(line);
                output.extend_from_slice("│ ".as_bytes());
                output.extend_from_slice(line.as_bytes());
                output.push(b'\n');
            }
        }
        output.extend_from_slice("└─ end preview\n".as_bytes());
        Ok(output)
    }
}

#[derive(Debug, Default)]
pub struct SourceRenderer;

impl BlockRenderer for SourceRenderer {
    fn render(
        &mut self,
        block: &SemanticBlock,
        _context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError> {
        Ok(block.source().to_vec())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalRendererConfig {
    pub renderer_id: String,
    pub program: OsString,
    pub arguments: Vec<OsString>,
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

impl ExternalRendererConfig {
    pub fn new(renderer_id: impl Into<String>, program: impl Into<OsString>) -> Self {
        Self {
            renderer_id: renderer_id.into(),
            program: program.into(),
            arguments: Vec::new(),
            timeout: Duration::from_secs(5),
            max_output_bytes: 4 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub struct ExternalRenderer {
    config: ExternalRendererConfig,
}

impl ExternalRenderer {
    pub fn new(config: ExternalRendererConfig) -> Self {
        Self { config }
    }

    fn run_engine(
        &self,
        block: &SemanticBlock,
        context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError> {
        let mut command = Command::new(&self.config.program);
        command
            .args(&self.config.arguments)
            .env("PTYMARK_RENDERER_PROTOCOL", "stdio-v1")
            .env("PTYMARK_RENDERER_ID", &self.config.renderer_id)
            .env("PTYMARK_BLOCK_KIND", block.kind().as_str())
            .env("PTYMARK_SOURCE_BYTES", block.source().len().to_string())
            .env("PTYMARK_COLOR", if context.color { "1" } else { "0" })
            .env(
                "PTYMARK_TERMINAL_WIDTH",
                context
                    .terminal_width
                    .map(|width| width.to_string())
                    .unwrap_or_default(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        let mut child = command.spawn().map_err(|error| {
            RenderError::new(format!(
                "renderer `{}` could not start `{}`: {error}",
                self.config.renderer_id,
                self.config.program.to_string_lossy()
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

        let body = block.body().to_vec();
        let stdin_writer = thread::spawn(move || {
            let mut stdin = stdin;
            let result = stdin.write_all(&body);
            drop(stdin);
            result
        });
        let stdout_limit = self.config.max_output_bytes;
        let stdout_reader = thread::spawn(move || read_capped(stdout, stdout_limit));
        let stderr_reader = thread::spawn(move || read_capped(stderr, STDERR_LIMIT_BYTES));

        let status = match wait_with_timeout(&mut child, self.config.timeout) {
            Ok(Some(status)) => status,
            Ok(None) => {
                return Err(RenderError::new(format!(
                    "renderer `{}` exceeded timeout of {} ms",
                    self.config.renderer_id,
                    self.config.timeout.as_millis()
                )));
            }
            Err(error) => {
                terminate_child(&mut child);
                return Err(RenderError::new(format!(
                    "renderer `{}` wait failed: {error}",
                    self.config.renderer_id
                )));
            }
        };

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
                "renderer `{}` output exceeded {} bytes",
                self.config.renderer_id, self.config.max_output_bytes
            )));
        }

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
                self.config.renderer_id
            )));
        }

        Ok(stdout_result.bytes)
    }
}

impl BlockRenderer for ExternalRenderer {
    fn render(
        &mut self,
        block: &SemanticBlock,
        context: &RenderContext,
    ) -> Result<Vec<u8>, RenderError> {
        self.run_engine(block, context)
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
        if retained < count {
            overflowed = true;
        }
    }

    Ok(CappedRead { bytes, overflowed })
}

fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    {
        let process_group = format!("-{}", child.id());
        let _ = Command::new("/bin/kill")
            .arg("-KILL")
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

#[cfg(test)]
mod tests {
    use super::{
        BlockRenderer, ExternalRenderer, ExternalRendererConfig, PreviewRenderer, RenderContext,
        SourceRenderer,
    };
    use crate::model::{BlockKind, SemanticBlock};
    use std::ffi::OsString;
    use std::time::Duration;

    fn block() -> SemanticBlock {
        SemanticBlock::new(
            BlockKind::Mermaid,
            b"```mermaid\nA --> B\n```\n".to_vec(),
            b"A --> B\n".to_vec(),
        )
    }

    #[test]
    fn preview_renderer_emits_display_bytes_without_source_fence() {
        let mut renderer = PreviewRenderer;
        let rendered = renderer
            .render(&block(), &RenderContext::default())
            .expect("render");
        let text = String::from_utf8(rendered).expect("UTF-8 preview");
        assert!(text.contains("mermaid preview"));
        assert!(text.contains("A --> B"));
        assert!(!text.contains("```mermaid"));
    }

    #[test]
    fn source_renderer_is_lossless() {
        let block = block();
        let mut renderer = SourceRenderer;
        let rendered = renderer
            .render(&block, &RenderContext::default())
            .expect("render");
        assert_eq!(rendered, block.source());
    }

    #[cfg(unix)]
    #[test]
    fn external_renderer_uses_stdio_protocol() {
        let mut config = ExternalRendererConfig::new("test/cat", "/bin/sh");
        config.arguments = vec![OsString::from("-c"), OsString::from("cat")];
        let mut renderer = ExternalRenderer::new(config);
        let rendered = renderer
            .render(&block(), &RenderContext::default())
            .expect("external render");
        assert_eq!(rendered, b"A --> B\n");
    }

    #[cfg(unix)]
    #[test]
    fn external_renderer_enforces_timeout() {
        let mut config = ExternalRendererConfig::new("test/sleep", "/bin/sh");
        config.arguments = vec![OsString::from("-c"), OsString::from("sleep 1")];
        config.timeout = Duration::from_millis(20);
        let mut renderer = ExternalRenderer::new(config);
        let error = renderer
            .render(&block(), &RenderContext::default())
            .expect_err("timeout must fail");
        assert!(error.to_string().contains("timeout"));
    }

    #[cfg(unix)]
    #[test]
    fn external_renderer_enforces_output_limit() {
        let mut config = ExternalRendererConfig::new("test/output", "/bin/sh");
        config.arguments = vec![OsString::from("-c"), OsString::from("printf 12345")];
        config.max_output_bytes = 4;
        let mut renderer = ExternalRenderer::new(config);
        let error = renderer
            .render(&block(), &RenderContext::default())
            .expect_err("output limit must fail");
        assert!(error.to_string().contains("exceeded 4 bytes"));
    }
}
