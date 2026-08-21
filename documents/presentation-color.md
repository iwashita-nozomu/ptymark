<!--
@dependency-start
contract design
responsibility Defines terminal-cell legibility, background-safe symbol presentation, and color-policy behavior.
upstream implementation ../src/runtime.rs color-policy resolution
upstream implementation ../renderers/managed/ansi-presenter.mjs bounded terminal-cell presentation
downstream verification ../tests/managed_presenter_contrast.mjs geometry, coverage, dark/light, and tmux regression
downstream verification ../tests/presentation_contrast_contract.rs CLI policy and exact-source fallback regression
@dependency-end
-->

# Terminal presentation geometry, legibility, and contrast

Ptymark treats SVG generation and terminal presentation as separate success boundaries. A valid MathJax or Mermaid SVG is not a successful terminal result unless presentation produces terminal-visible, structurally usable cells. Non-empty bytes alone are insufficient.

## Terminal-cell geometry

The managed presenter preserves the SVG aspect ratio in physical terminal space rather than treating a character cell as square. Let:

- `C` be the requested terminal columns;
- `a = cell_width / cell_height` be the terminal-cell aspect ratio;
- `r = svg_height / svg_width` be the source aspect ratio; and
- `H_half` be the number of vertical half-cells used by `▀`, `▄`, and `█`.

Because one terminal row contains two vertical half-cells, the aspect-preserving height is

```text
H_half = round(2 C a r).
```

The default `a = 0.5` models a terminal cell whose height is twice its width. `H_half` is bounded to `[2, 1024]`; the presenter does not grow an unbounded raster for an unusually tall artifact.

Each half-cell is rasterized at `s × s` samples before reduction. The default is `s = 4`, with a supported range of `[2, 4]`:

```text
raster_width  = s C
raster_height = s H_half.
```

Supersampling is internal. The emitted terminal width remains exactly `C`, and tmux, SSH, `TERM`, dark/light appearance, and redirected output do not alter the geometry.

## Stroke coverage and legibility boundary

A raster sample is first classified relative to the transparent or uniform opaque SVG background. For each terminal half-cell, the presenter computes alpha-weighted foreground coverage

```text
coverage = foreground alpha mass / s².
```

Coverage below `0.10` is discarded. At the default `s = 4`, one isolated full-alpha raster sample covers `1/16 = 0.0625` and is removed, while a one-sample-wide stroke crossing the half-cell contributes up to `4/16 = 0.25` and remains representable. This preserves thin MathJax bars and stems without turning isolated antialiasing noise into terminal blocks.

After coverage reduction, a presentation with at least eight occupied half-cells is rejected when more than `0.30` of them have no occupied neighbor in their surrounding 3×3 neighborhood. This is a bounded structural check, not OCR or formula recognition. It distinguishes connected strokes from the sparse block mosaic that motivated Issue #167. Rejection uses the existing renderer-failure path, so normal non-strict rendering restores visible exact source instead of displaying a misleading artifact.

## Bounded policy and diagnostics

The managed presenter owns three optional environment controls:

| Variable | Default | Accepted values | Effect |
| --- | ---: | --- | --- |
| `PTYMARK_CELL_ASPECT` | `0.5` | finite number in `[0.25, 1]` | terminal `cell_width / cell_height` used by the height equation |
| `PTYMARK_RASTER_SCALE` | `4` | integer in `[2, 4]` | samples per terminal half-cell axis |
| `PTYMARK_PRESENTER_DIAGNOSTICS` | unset | `1` enables | writes resolved terminal/raster dimensions, aspect, scale, occupied half-cells, and isolated ratio to stderr |

Invalid aspect or scale values fail closed rather than silently changing geometry. Diagnostics never change stdout or the rendered bytes.

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

This rule changes only presentation color. It does not alter exact semantic source, terminal geometry, or renderer routing.

## Visibility and legibility failure fallback

An all-transparent or uniform all-background raster, an artifact whose occupied cells all fail the contrast rule after correction, or a terminal-cell raster that exceeds the bounded fragmentation ratio is a presentation failure.

- The normal non-strict pipeline restores the complete exact source block, including adjacent Japanese or other text.
- `--strict` reports the renderer failure instead of claiming presentation success.
- `--source` detects complete blocks and commits their exact source bytes without starting external engines.
- `--safe` bypasses semantic detection and rendering, preserving the complete input byte-for-byte.

The presenter has no tmux-specific rendering branch. Normal PTY, tmux, SSH, and redirected output receive the same symbol geometry; only explicit color policy and the optional appearance hint can change foreground SGR bytes.
