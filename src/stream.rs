use crate::pipeline::{DisplayPipeline, MAX_PENDING_OUTPUT_BYTES, PipelineError};
use crate::render::RenderCancellation;
use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread;

const BUFFER_BYTES: usize = 8192;
const PENDING_CHUNKS: usize = MAX_PENDING_OUTPUT_BYTES.div_ceil(BUFFER_BYTES);

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
    ReaderPanicked,
}

impl fmt::Display for PumpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "stream read failed: {error}"),
            Self::Pipeline(error) => error.fmt(formatter),
            Self::Flush(error) => write!(formatter, "display flush failed: {error}"),
            Self::ReaderPanicked => formatter.write_str("stream reader thread panicked"),
        }
    }
}

impl Error for PumpError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read(error) | Self::Flush(error) => Some(error),
            Self::Pipeline(error) => Some(error),
            Self::ReaderPanicked => None,
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

    /// Read child output on a bounded queue while rendering happens on the
    /// display thread. When the queue reaches one MiB, the current external
    /// render attempt is cancelled. The reader then blocks until exact-source
    /// fallback releases the queued bytes in their original order.
    pub(crate) fn run_bounded_with_updates(
        self,
        reader: &mut (dyn Read + Send),
        display: &mut dyn Write,
        pipeline: &mut DisplayPipeline,
        mut update_context: impl FnMut(&mut DisplayPipeline),
        mut cancel_source: impl FnMut(),
    ) -> Result<(), PumpError> {
        let cancellation = pipeline.cancellation_handle();
        thread::scope(|scope| {
            let (sender, receiver) = mpsc::sync_channel(PENDING_CHUNKS);
            let reader_cancellation = cancellation.clone();
            let reader_handle =
                scope.spawn(move || read_bounded(self, reader, sender, reader_cancellation));

            let processing = (|| -> Result<(), PumpError> {
                loop {
                    update_context(pipeline);
                    match receiver.recv() {
                        Ok(ReadMessage::Bytes(bytes)) => {
                            update_context(pipeline);
                            pipeline
                                .feed(&bytes, display)
                                .map_err(PumpError::Pipeline)?;
                            if self.flush_policy == FlushPolicy::AfterEachChunk {
                                display.flush().map_err(PumpError::Flush)?;
                            }
                        }
                        Ok(ReadMessage::End) => break,
                        Ok(ReadMessage::Error(error)) => return Err(PumpError::Read(error)),
                        Err(_) => break,
                    }
                }
                update_context(pipeline);
                pipeline.finish(display).map_err(PumpError::Pipeline)
            })();

            if processing.is_err() {
                cancellation.cancel();
                cancel_source();
            }
            drop(receiver);
            let reader_result = reader_handle
                .join()
                .map_err(|_| PumpError::ReaderPanicked)?;
            processing?;
            reader_result
        })
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

#[derive(Debug)]
enum ReadMessage {
    Bytes(Vec<u8>),
    End,
    Error(io::Error),
}

fn read_bounded(
    pump: PipelinePump,
    reader: &mut (dyn Read + Send),
    sender: SyncSender<ReadMessage>,
    cancellation: RenderCancellation,
) -> Result<(), PumpError> {
    let mut buffer = [0_u8; BUFFER_BYTES];
    loop {
        let count = match reader.read(&mut buffer) {
            Ok(0) => {
                let _ = sender.send(ReadMessage::End);
                return Ok(());
            }
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if pump.accepts_as_end_of_stream(&error) => {
                let _ = sender.send(ReadMessage::End);
                return Ok(());
            }
            Err(error) => {
                let kind = error.kind();
                let message = error.to_string();
                let _ = sender.send(ReadMessage::Error(io::Error::new(kind, message.clone())));
                return Err(PumpError::Read(io::Error::new(kind, message)));
            }
        };
        let message = ReadMessage::Bytes(buffer[..count].to_vec());
        match sender.try_send(message) {
            Ok(()) => {}
            Err(TrySendError::Full(message)) => {
                cancellation.cancel();
                if sender.send(message).is_err() {
                    return Ok(());
                }
            }
            Err(TrySendError::Disconnected(_)) => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_PENDING_OUTPUT_BYTES, PipelinePump};
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

    #[test]
    fn pending_output_contract_is_one_mebibyte() {
        assert_eq!(MAX_PENDING_OUTPUT_BYTES, 1024 * 1024);
    }

    #[test]
    fn bounded_pump_preserves_order_for_an_in_memory_stream() {
        let config = Config::default();
        let mut pipeline = PipelineFactory::new(&config).build(PipelineOptions::default());
        let mut source = Cursor::new(b"A\n$$\nE = mc^2\n$$\nC\n".to_vec());
        let mut output = Vec::new();
        let mut cancelled = false;

        PipelinePump::standard()
            .run_bounded_with_updates(
                &mut source,
                &mut output,
                &mut pipeline,
                |_| {},
                || cancelled = true,
            )
            .expect("bounded pump");

        let text = String::from_utf8(output).expect("UTF-8");
        assert!(text.starts_with("A\n"));
        assert!(text.contains("ptymark math"));
        assert!(text.ends_with("C\n"));
        assert!(!cancelled);
    }
}
