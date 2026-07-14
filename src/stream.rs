use crate::pipeline::{DisplayPipeline, PipelineError};
use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};

const BUFFER_BYTES: usize = 8192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FlushPolicy {
    OnFinish,
    AfterEachChunk,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EndOfStreamPolicy {
    Standard,
    PseudoTerminal,
}

#[derive(Debug)]
pub(crate) enum PumpError {
    Read(io::Error),
    Pipeline(PipelineError),
    Flush(io::Error),
}

impl fmt::Display for PumpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "stream read failed: {error}"),
            Self::Pipeline(error) => error.fmt(formatter),
            Self::Flush(error) => write!(formatter, "display flush failed: {error}"),
        }
    }
}

impl Error for PumpError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read(error) | Self::Flush(error) => Some(error),
            Self::Pipeline(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PipelinePump {
    flush_policy: FlushPolicy,
    end_of_stream: EndOfStreamPolicy,
}

impl PipelinePump {
    pub(crate) const fn standard() -> Self {
        Self {
            flush_policy: FlushPolicy::OnFinish,
            end_of_stream: EndOfStreamPolicy::Standard,
        }
    }

    pub(crate) const fn interactive() -> Self {
        Self {
            flush_policy: FlushPolicy::AfterEachChunk,
            end_of_stream: EndOfStreamPolicy::PseudoTerminal,
        }
    }

    pub(crate) fn run(
        self,
        reader: &mut dyn Read,
        display: &mut dyn Write,
        pipeline: &mut DisplayPipeline,
    ) -> Result<(), PumpError> {
        self.run_with_updates(reader, display, pipeline, |_| {})
    }

    pub(crate) fn run_with_updates(
        self,
        reader: &mut dyn Read,
        display: &mut dyn Write,
        pipeline: &mut DisplayPipeline,
        mut update_context: impl FnMut(&mut DisplayPipeline),
    ) -> Result<(), PumpError> {
        let mut buffer = [0_u8; BUFFER_BYTES];

        loop {
            update_context(pipeline);
            let count = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if self.accepts_as_end_of_stream(&error) => break,
                Err(error) => return Err(PumpError::Read(error)),
            };

            update_context(pipeline);
            pipeline
                .feed(&buffer[..count], display)
                .map_err(PumpError::Pipeline)?;
            if self.flush_policy == FlushPolicy::AfterEachChunk {
                display.flush().map_err(PumpError::Flush)?;
            }
        }

        update_context(pipeline);
        pipeline.finish(display).map_err(PumpError::Pipeline)
    }

    fn accepts_as_end_of_stream(self, error: &io::Error) -> bool {
        if self.end_of_stream != EndOfStreamPolicy::PseudoTerminal {
            return false;
        }
        if matches!(
            error.kind(),
            io::ErrorKind::UnexpectedEof | io::ErrorKind::BrokenPipe
        ) {
            return true;
        }

        #[cfg(unix)]
        if error.raw_os_error() == Some(libc::EIO) {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::PipelinePump;
    use crate::config::Config;
    use crate::runtime::{PipelineFactory, PipelineOptions};
    use std::io::{self, Cursor, Read};

    struct InterruptedOnce<R> {
        inner: R,
        interrupted: bool,
    }

    impl<R: Read> Read for InterruptedOnce<R> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(io::Error::new(io::ErrorKind::Interrupted, "retry"));
            }
            self.inner.read(buffer)
        }
    }

    #[test]
    fn standard_pump_retries_interrupted_reads_and_finishes_the_pipeline() {
        let config = Config::default();
        let mut pipeline = PipelineFactory::new(&config).build(PipelineOptions::default());
        let source = Cursor::new(b"before\n$$\nE = mc^2\n$$\nafter\n".to_vec());
        let mut reader = InterruptedOnce {
            inner: source,
            interrupted: false,
        };
        let mut output = Vec::new();

        PipelinePump::standard()
            .run(&mut reader, &mut output, &mut pipeline)
            .expect("pump");

        let text = String::from_utf8(output).expect("UTF-8");
        assert!(text.starts_with("before\n"));
        assert!(text.contains("ptymark math"));
        assert!(text.ends_with("after\n"));
    }
}
