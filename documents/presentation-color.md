<!--
@dependency-start
contract design
responsibility Defines terminal-background-safe symbol presentation and color-policy behavior.
upstream implementation ../src/runtime.rs color-policy resolution
upstream implementation ../renderers/managed/ansi-presenter.mjs contrast-safe managed presentation
downstream verification ../tests/managed_presenter_contrast.mjs dark/light/tmux contrast regression
downstream verification ../tests/presentation_contrast_contract.rs CLI policy and exact-source fallback regression
@dependency-end
-->

# Terminal presentation color and contrast

Ptymark treats SVG generation and terminal presentation as separate success boundaries. A valid MathJax or Mermaid SVG is not a successful terminal result unless presentation produces at least one terminal-visible cell. Non-empty bytes alone are insufficient.

## Color policy

`profiles.<name>.presentation.color` has three deterministic values. Ptymark does not infer terminal background from `TERM`, tmux, SSH, or whether stdout is redirected.

| Profile value | `--color` absent | `--color` present |
| --- | --- | --- |
| `auto` | terminal-default foreground/background; no color SGR | contrast-safe truecolor foreground |
| `always` | contrast-safe truecolor foreground | contrast-safe truecolor foreground |
| `never` | terminal-default foreground/background; no color SGR | terminal-default foreground/background; no color SGR |

`--color` is therefore an explicit opt-in only while the selected profile is `auto`. It cannot override `always` or `never`.

The managed presenter never emits an RGB background (`48;2;...`). It treats transparent pixels, or a uniform opaque color observed at least three raster corners, as the artifact background. Symbol occupancy is then the alpha- or linear-RGB-distinguishable foreground relative to that background rather than source luminance. Black strokes therefore do not become spaces on a dark terminal, white strokes do not disappear on a light terminal, and an opaque SVG canvas does not become a solid block. With color disabled, block symbols use the terminal's default foreground and background.

## Contrast rule

For color output, the managed presenter uses WCAG relative luminance and requires a contrast ratio of at least 4.5:1 between every emitted foreground and the expected terminal background.

- `PTYMARK_APPEARANCE=dark` checks against ideal black and replaces an insufficient foreground with white.
- `PTYMARK_APPEARANCE=light` checks against ideal white and replaces an insufficient foreground with black.
- An absent or unsupported appearance uses `#757575`, which remains above 4.5:1 against both ideal black and ideal white.

This rule changes only presentation color. It does not alter the exact semantic source, SVG artifact, terminal width, or renderer routing.

## Visibility failure and fallback

An all-transparent or uniform all-background raster, or an artifact whose occupied cells all fail the contrast rule after correction, is a presentation failure.

- The normal non-strict pipeline restores the complete exact source block.
- `--strict` reports the renderer failure instead of claiming presentation success.
- `--source` detects complete blocks and commits their exact source bytes without starting external engines.
- `--safe` bypasses semantic detection and rendering, preserving the complete input byte-for-byte.

The presenter has no tmux-specific rendering branch. Normal PTY, tmux, and SSH receive the same symbol and SGR bytes; only the explicit color policy and optional appearance hint affect those bytes.
