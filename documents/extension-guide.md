# ptymark Extension Guide

<!--
@dependency-start
contract design
responsibility Explains how to extend engines, detectors, presenters, caches, and runtime composition without violating terminal invariants.
upstream design ./system-design.md defines provider and runtime boundaries
downstream implementation ../src/runtime.rs exposes provider traits and RuntimeBuilder
downstream test ../tests/runtime_contract.rs verifies custom providers
@dependency-end
-->

## 1. Extension philosophy

`ptymark` is designed as a host for independently replaceable services rather than a monolithic renderer. An extension should add one capability without teaching unrelated parts about it.

A valid extension:

- depends on typed contracts, not raw TOML tables;
- never writes directly to display stdout;
- never receives terminal-control bytes unless it is a safety/transport component;
- preserves exact source fallback;
- declares stable identity and version information;
- is bounded in memory, time, process output, and queued work;
- can be absent without breaking source-only operation;
- participates in tests and runtime build reporting.

## 2. Extension catalog

The runtime composition root accepts four provider categories.

```text
RuntimeBuilder
├─ DetectorProvider
├─ EngineProvider[]
├─ CacheProvider
└─ PresenterProvider
```

Embedding applications construct a builder, register providers, then build a session from a `ConfigSnapshot`.

```rust
use ptymark::{ConfigSnapshot, RuntimeBuilder, RuntimeRequest};

fn build(snapshot: ConfigSnapshot) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = RuntimeBuilder::default()
        .with_engine_provider(MyEngineProvider::new())?
        .build(snapshot, RuntimeRequest::preview())?;

    println!("{}", runtime.plan().summary());
    Ok(())
}
```

The exact public API is intentionally small. Providers own construction details and return trait objects through stable host interfaces.

## 3. Adding a rendering engine

### 3.1 Implement `RenderEngine`

An engine exposes a descriptor before it is called.

```rust
use ptymark::{
    ArtifactFormat, BlockKind, EngineDescriptor, ExecutionModel,
    LayoutSensitivity, RenderArtifact, RenderEngine, RenderError, RenderRequest,
};

struct MyMathEngine {
    descriptor: EngineDescriptor,
}

impl MyMathEngine {
    fn new() -> Self {
        Self {
            descriptor: EngineDescriptor::new(
                "example/math-svg",
                "1.0.0",
                vec![BlockKind::Math],
                vec![ArtifactFormat::Svg],
                LayoutSensitivity::Columns,
                ExecutionModel::InProcess,
            ),
        }
    }
}

impl RenderEngine for MyMathEngine {
    fn descriptor(&self) -> &EngineDescriptor {
        &self.descriptor
    }

    fn render(
        &mut self,
        request: &RenderRequest<'_>,
    ) -> Result<RenderArtifact, RenderError> {
        let svg = render_with_existing_library(request.block.body())?;
        Ok(RenderArtifact::new(
            ArtifactFormat::Svg,
            svg,
            self.descriptor.identity.clone(),
            request.block.kind(),
            self.descriptor.layout_sensitivity,
        ))
    }
}
```

Rules:

- IDs use a namespaced stable form such as `vendor/engine`;
- versions change whenever output compatibility or protocol changes;
- the artifact engine identity must equal the descriptor identity;
- the artifact kind must equal the request kind;
- engines return bytes; they do not emit terminal protocol escape sequences;
- an engine must not mutate global terminal state.

### 3.2 Register through an `EngineProvider`

```rust
use ptymark::{EngineProvider, EngineRegistry, RuntimeBuildContext, RuntimeBuildReport};

struct MyEngineProvider;

impl EngineProvider for MyEngineProvider {
    fn id(&self) -> &str {
        "example/engines"
    }

    fn register(
        &self,
        _context: &RuntimeBuildContext<'_>,
        registry: &mut EngineRegistry,
        report: &mut RuntimeBuildReport,
    ) -> Result<(), ptymark::RuntimeBuildError> {
        registry.register(MyMathEngine::new())?;
        report.registered_engine("example/math-svg");
        Ok(())
    }
}
```

Provider registration is deterministic. Duplicate engine IDs fail instead of silently replacing an existing implementation.

### 3.3 Configure candidate order

```toml
[profiles.interactive.engines.math]
candidates = ["example/math-svg", "mathjax-worker", "katex", "source"]
preferred_artifacts = ["image/svg+xml", "application/mathml+xml", "text/plain"]
```

The source candidate remains last. An unavailable optional engine is reported and skipped.

## 4. Adding a process engine

Prefer `ProcessEngine` rather than invoking a shell.

```rust
use ptymark::{
    ArtifactFormat, BlockKind, ExecutionModel, LayoutSensitivity,
    ProcessEngine, ProcessEngineConfig,
};
use std::path::PathBuf;
use std::time::Duration;

let config = ProcessEngineConfig {
    id: "example/diagram".into(),
    version: "2".into(),
    supported_kinds: vec![BlockKind::Mermaid],
    formats: vec![ArtifactFormat::Svg],
    layout_sensitivity: LayoutSensitivity::Pixels,
    execution_model: ExecutionModel::OneShotProcess,
    program: PathBuf::from("/opt/example/render-diagram"),
    arguments: vec!["--format".into(), "svg".into()],
    timeout: Duration::from_millis(1500),
    max_stdout_bytes: 8 * 1024 * 1024,
    max_stderr_bytes: 64 * 1024,
    working_directory: None,
    environment: Default::default(),
    inherit_environment: vec!["PATH".into(), "LANG".into()],
};
let engine = ProcessEngine::new(config)?;
```

The process protocol is:

```text
stdin   exact semantic body
stdout  one artifact
stderr  bounded diagnostic text
```

Host-provided environment variables include:

```text
PTYMARK_RENDERER_PROTOCOL=stdio-v1
PTYMARK_RENDERER_ID
PTYMARK_BLOCK_KIND
PTYMARK_SOURCE_BYTES
PTYMARK_COLOR
PTYMARK_TERMINAL_WIDTH
```

Security requirements:

- user-configured programs are absolute paths;
- no shell string is accepted;
- inherited environment is an explicit allowlist;
- working directories are absolute;
- stdout/stderr/time are bounded independently;
- normal render operations never install software or access a package manager.

## 5. Adding a presenter

A presenter converts an artifact into terminal-safe display bytes.

```rust
use ptymark::{
    ArtifactFormat, ArtifactPresenter, RenderArtifact, RenderError,
    SemanticBlock, TerminalCapabilities,
};

struct MyImagePresenter {
    accepted: [ArtifactFormat; 1],
}

impl ArtifactPresenter for MyImagePresenter {
    fn id(&self) -> &str {
        "example/image-protocol-v1"
    }

    fn accepted_formats(&self) -> &[ArtifactFormat] {
        &self.accepted
    }

    fn present(
        &mut self,
        artifact: &RenderArtifact,
        source: &SemanticBlock,
        capabilities: TerminalCapabilities,
    ) -> Result<Vec<u8>, RenderError> {
        if !capabilities.inline_images {
            return Ok(source.source().to_vec());
        }
        encode_verified_protocol(artifact)
    }
}
```

Presenter requirements:

- only emit a protocol confirmed by `TerminalCapabilities`;
- scope image IDs to ptymark-owned objects;
- never erase unrelated terminal images;
- preserve an exact source fallback;
- include capability and policy changes in the cache fingerprint;
- avoid input/mouse/termios changes.

A future presenter registry may select among multiple compatible presenters. The initial runtime chooses one provider per session.

## 6. Adding a cache backend

Implement `ArtifactCache`.

```rust
use ptymark::{
    ArtifactCache, ArtifactCacheKey, CacheAdmission, CacheStats,
    InvalidationScope, RenderArtifact,
};

struct MyCache;

impl ArtifactCache for MyCache {
    fn get(&mut self, key: &ArtifactCacheKey) -> Option<RenderArtifact> {
        // Verify key schema and artifact integrity before returning.
        None
    }

    fn insert(
        &mut self,
        key: ArtifactCacheKey,
        artifact: RenderArtifact,
    ) -> CacheAdmission {
        CacheAdmission::Rejected
    }

    fn invalidate(&mut self, scope: &InvalidationScope) -> usize {
        0
    }

    fn clear(&mut self) {}

    fn stats(&self) -> CacheStats {
        CacheStats::default()
    }
}
```

Persistent implementations additionally require:

- a versioned cache-key schema;
- atomic write/rename;
- corruption detection;
- permission checks;
- bounded disk use and eviction;
- private-mode hard disable;
- engine/version and presenter invalidation;
- no storage of failed, partial, cancelled, or stale-generation artifacts.

A backend must not assume that the existing 64-bit in-memory source fingerprint is collision-resistant. Persistent caches should use a cryptographic digest.

## 7. Adding a detector

Implement `SemanticDetector` and register it through a `DetectorProvider`.

A detector receives only bytes classified as safe text by the terminal output gate. It must still be conservative.

Requirements:

- explicit opening and closing boundaries;
- chunk-boundary independence;
- exact source retention;
- bounded line and total source sizes;
- exact passthrough on overflow, ambiguity, or finish-without-close;
- no interpretation of terminal escape sequences;
- no assumption that input is a complete document.

General interactive profiles must not enable inline `$...$`, arbitrary Markdown headings, or heuristic code detection. A document-only detector should use a distinct profile and provider ID.

## 8. Adding a new semantic kind

A new semantic kind affects configuration, detector output, engine descriptors, documentation, and compatibility. Treat it as a schema-level feature rather than a local enum edit.

Required design work:

1. define an unambiguous source boundary;
2. define exact source/body representation;
3. define default fallback;
4. define supported artifact media types;
5. define cache/layout inputs;
6. define security and dependency implications;
7. add schema migration/version rules;
8. add terminal-safety and chunk-boundary tests;
9. add user documentation and examples.

Code-only embedding extensions may stay outside the TOML schema, but user-authored configuration requires a reviewed schema change.

## 9. Runtime build reporting

Extensions should make availability explainable.

A provider records one of:

```text
registered
unavailable(reason)
disabled-by-profile
rejected-invalid-configuration
```

Reasons may include missing executable, missing bundle, incompatible protocol, unsupported platform, or failed capability requirement. Reports must not include secret values or semantic source.

## 10. Compatibility checklist

Before publishing an extension:

- [ ] stable provider and engine/presenter IDs are documented;
- [ ] version changes invalidate incompatible cache entries;
- [ ] artifact media types are exact;
- [ ] no stdout side effects occur outside presenter output;
- [ ] source fallback is tested;
- [ ] timeout/output/memory bounds are tested;
- [ ] terminal-control input cannot reach the extension;
- [ ] private mode does not persist source;
- [ ] missing dependencies are diagnosable;
- [ ] installation is explicit and offline use is possible afterward;
- [ ] licenses and redistributed assets are documented;
- [ ] Linux/macOS and canonical Docker checks pass.

## 11. Review expectations

Extensions that touch the transport or safety plane require a dedicated design review. Engine, presenter, and cache extensions can normally be reviewed independently when they honor the contracts above.

Any proposal that weakens byte-exact passthrough, executes project configuration implicitly, emits an unverified terminal protocol, or installs dependencies during render is outside the accepted extension model.
