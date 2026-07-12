<!--
@dependency-start
contract design
responsibility Defines ptymark's display-only interception, engine orchestration, artifact presentation, cache, and extension contracts.
upstream design ./architecture.md defines the pre-display product boundary.
upstream design ./ui-design.md defines viewport, resize, image, and cache behavior.
downstream implementation ../src/terminal.rs implements the output safety gate.
downstream implementation ../src/coordinator.rs implements engine selection and cache orchestration.
downstream implementation ../src/cache.rs implements replaceable artifact caches.
downstream workflow ../.github/workflows/ptymark-ci.yml validates contracts and latency budgets.
@dependency-end
-->

# Renderer architecture

## Stable product boundary

`ptymark` owns only the child-process output path immediately before bytes are committed to the
terminal emulator. It does not replace terminal input, PTY signaling, termios, window-size
propagation, process lifecycle, shell integration, or the terminal emulator's screen model.

```text
keyboard / mouse / paste / signals / resize
    └──────────────────────────────────────────────▶ child PTY (unchanged)

child PTY output
    ▶ TerminalOutputGate
    ▶ SemanticDetector
    ▶ RenderCoordinator
    ▶ ArtifactPresenter
    ▶ display writer
    ▶ terminal emulator
```

Terminal control bytes have priority over semantic rendering. ANSI, OSC, DCS, APC, carriage-return
updates, unknown escape sequences, and alternate-screen output are emitted byte-for-byte. Encountering
a control region flushes any uncommitted semantic candidate as source before bypassing the detector.

## Object model

### `TerminalOutputGate`

Classifies output into `SafeText` and `RawTerminalBytes`. It is deliberately not a terminal emulator.
It tracks only enough control-sequence state to preserve sequence boundaries and detect alternate
screen entry and exit. Raw bytes are never normalized or regenerated.

### `SemanticDetector`

Recognizes explicit, closed semantic blocks. The initial implementation recognizes line-bounded
Mermaid fences and `$$` math blocks. Ordinary output is released as soon as it cannot be an opener;
a shell prompt is not held until newline.

### `EngineDescriptor`

Declares engine identity and version, supported block kinds, artifact formats, layout sensitivity,
and execution model. Descriptor data participates in cache keys and extension selection.

### `EngineRegistry`

Owns engine instances and their lifecycle. The stream loop does not contain hard-coded engine
matches. A downstream engine is added by registration and selector policy, not by changing the
terminal gate or detector.

### `EngineSelector`

Returns an ordered candidate list for a semantic kind and the presenter's accepted formats. This
allows a persistent real-time engine, a one-shot correctness fallback, and a source fallback to be
configured independently.

### `RenderCoordinator`

Coordinates one semantic render request:

1. obtain ordered engine candidates;
2. reject incompatible kind or artifact-format candidates;
3. construct a versioned, viewport-aware cache key;
4. return a cache hit without executing an engine;
5. execute candidates in order and record attempt duration and diagnostics;
6. validate engine identity and artifact format;
7. cache successful, cacheable artifacts only;
8. return an artifact without writing to the terminal.

The coordinator does not parse terminal bytes and does not emit terminal escape sequences.

### `ArtifactCache`

A replaceable optimization boundary. `MemoryArtifactCache` is a bounded LRU implementation;
`NoopArtifactCache` provides private/no-cache mode. Future disk and tiered caches implement the same
interface.

The key includes source fingerprint, block kind, engine identity/version, artifact format, layout
sensitivity and viewport, theme/options fingerprints, presenter identity, and terminal capability
fingerprint. Failed, cancelled, timed-out, stale-generation, or explicitly non-cacheable results are
not admitted.

### `ArtifactPresenter`

Maps an artifact to display bytes supported by the active terminal and transport. Engines never write
to stdout. The initial text presenter accepts terminal text, source, and MathML. Image presenters for
Kitty Graphics, iTerm2 Inline Images, and Sixel are separate extensions. Unsupported presentation
falls back through the normal pre-display source path.

### `DisplayInterceptor`

Composes the output gate and pre-display renderer. `SafeText` reaches semantic detection;
`RawTerminalBytes` switches the pre-display layer to bypass after flushing pending source. This is
the sole display interception object used by the preview path and the future PTY host.

## Selected existing engines

| Semantic input | Primary real-time engine | Correctness/fallback path | Artifact |
| --- | --- | --- | --- |
| Mermaid | persistent worker using Mermaid CLI's exported renderer and reused Chromium | one-shot `mmdc` | SVG |
| TeX block math | persistent MathJax 4 worker | one-shot MathJax | SVG |
| Math comparator | KaTeX | none by default | MathML |
| Typst-native input | Typst CLI | source | SVG/PDF |

`ptymark` does not implement diagram layout or mathematical typesetting. JavaScript renderer
resolution is locked in `renderers/package-lock.json`; Typst and Rust are pinned separately.

## Performance contract

GitHub Actions is the canonical measurement environment. It records:

- persistent-worker warm latency;
- one-shot cold latency;
- p50, p95, maximum, and artifact bytes;
- Rust coordinator cache-hit latency;
- benchmark JSON as an Actions artifact and job summary.

The current CI safety budgets are intentionally wider than product targets to avoid runner noise:

- MathJax persistent p95: 500 ms;
- Mermaid persistent p95: 2000 ms;
- coordinator cache hit p95: 2 ms.

One-shot measurements are evidence, not interactive gates. Product targets can tighten after enough
historical Actions artifacts exist.

## Extension example

A downstream Mermaid engine registers a descriptor and updates selector policy:

```text
EngineDescriptor {
  id: "custom/remote-mermaid",
  version: "1",
  kinds: [mermaid],
  formats: [svg],
  execution: persistent-worker,
  layout: pixels
}

selector[mermaid] = [
  "custom/remote-mermaid",
  "mermaid-cli/11.16.0-persistent",
  "mermaid-cli/11.16.0-oneshot"
]
```

No changes are required in `TerminalOutputGate`, `SemanticDetector`, `ArtifactCache`, the WezTerm
plugin, or terminal input handling.

## Deferred runtime integration

The child PTY host, live resize generations, render cancellation, image placement, and persistent
disk cache remain tracked in Issues #3 and #4. Their implementations must preserve these interfaces
and the display-only boundary.
