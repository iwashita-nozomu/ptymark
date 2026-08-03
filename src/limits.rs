//! Internal, versioned safety and runtime limits.
//!
//! These constants define the alpha.4 safety floor. They are intentionally not
//! user-authored TOML: configuration may choose a stricter path, but it cannot
//! weaken process, parser, artifact, or terminal-output bounds.

use std::time::Duration;

pub(crate) const EXTERNAL_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const MAX_RENDER_ARTIFACT_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_PRESENTATION_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_RENDERER_DIAGNOSTIC_BYTES: usize = 64 * 1024;
pub(crate) const MAX_MATH_ARGUMENT_BYTES: usize = 32 * 1024;
pub(crate) const MAX_PENDING_TERMINAL_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_SEMANTIC_BLOCK_BYTES: usize = 1024 * 1024;

pub(crate) const MAX_OPENMATH_INPUT_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_OPENMATH_DEPTH: usize = 128;
pub(crate) const MAX_OPENMATH_NODES: u32 = 8192;
pub(crate) const MAX_SVG_NODES: u32 = 65_536;

pub(crate) const MAX_TERMINAL_CONTROL_BYTES: usize = 64 * 1024;
pub(crate) const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);
pub(crate) const RESIZE_POLL_INTERVAL: Duration = Duration::from_millis(80);
pub(crate) const DEFAULT_PTY_ROWS: u16 = 24;
#[cfg(windows)]
pub(crate) const CONPTY_OUTPUT_DRAIN_GRACE: Duration = Duration::from_millis(100);

// User-authored resource preferences are bounded independently from the hard
// renderer/process floor. They are configurable because they affect local
// performance, but cannot request unbounded memory or nonsensical dimensions.
pub(crate) const MAX_USER_CACHE_ENTRIES: usize = 4096;
pub(crate) const MAX_USER_CACHE_BYTES: usize = 512 * 1024 * 1024;
pub(crate) const MAX_FALLBACK_COLUMNS: u16 = 1000;
