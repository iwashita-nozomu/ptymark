pub mod detector;
pub mod model;
pub mod predisplay;
pub mod renderer;
pub mod ui;

pub use detector::{DetectError, FencedDetector, PassthroughDetector, SemanticDetector};
pub use model::{BlockKind, DisplayMode, SemanticBlock, StreamItem};
pub use predisplay::{PreDisplayError, PreDisplayRenderer, PreDisplayReport};
pub use renderer::{
    BlockRenderer, PreviewRenderer, RenderContext, RenderError, SourceRenderer,
};
pub use ui::{
    resize_action, stable_fingerprint, CachePolicy, CacheStats, LayoutSensitivity, RenderCache,
    RenderKey, ResizeAction, Viewport,
};
