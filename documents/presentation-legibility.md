<!--
@dependency-start
contract design
responsibility Defines bounded terminal geometry, stroke coverage, and legibility fallback for managed symbol presentation.
upstream implementation ../renderers/managed/ansi-presenter.mjs aspect-aware supersampled presentation
upstream implementation ./presentation-color.md terminal-background-safe contrast and exact-source fallback
downstream verification ../tests/managed_presenter_legibility.mjs geometry and stroke-coverage regression
downstream verification ../tests/managed_renderer_smoke.sh real MathJax and PTY smoke
@dependency-end
-->

# Terminal math legibility

Ptymark treats the requested terminal width as a maximum, not as an instruction to stretch every SVG across the whole terminal. The managed presenter first chooses a bounded terminal geometry, then rasterizes above terminal resolution and reduces each sample region to a half-block cell. This keeps common MathJax strokes recognizable without changing the contrast or fallback contracts.

## Geometry model

Let

- `C_max` be the requested maximum width from `--columns` / presenter `--size`;
- `w_s` and `h_s` be the browser-resolved SVG dimensions in CSS pixels;
- `d` be the source-pixel density per terminal cell;
- `a = h_cell / w_cell` be the terminal cell height-to-width ratio.

The managed presenter uses

```text
C = min(C_max, ceil(w_s / d))
R = ceil(C * (h_s / w_s) / a)
```

where `C` is the emitted terminal width and `R` is the terminal row count. The first equation prevents a short expression such as `E = mc^2` from being enlarged to all 80 columns. The second preserves physical aspect ratio because the displayed height-to-width ratio is approximately `R * a / C = h_s / w_s`; the only error is the unavoidable rounding of at most one terminal row.

Defaults and hard bounds are:

| Quantity | Default | Accepted range |
| --- | ---: | ---: |
| requested columns | profile / CLI value | 1–512 |
| `PTYMARK_SOURCE_PIXELS_PER_CELL` (`d`) | 8 | 4–16 |
| `PTYMARK_CELL_ASPECT_RATIO` (`a`) | 2.0 | 1.0–3.0 |
| terminal rows | derived | 1–512 |

These values are renderer-local presentation parameters. They do not change semantic source, MathJax input, renderer routing, or terminal transport. `PTYMARK_PRESENTER_REPORT=1` writes the resolved maximum width, fitted width, rows, aspect ratio, source-pixel density, and raster scale to stderr without changing stdout.

## Stroke coverage

A one-pixel-per-half-cell raster loses the area information carried by thin antialiased MathJax strokes. The managed presenter therefore rasterizes each terminal half-cell as an `s × s` sample region, with `s = PTYMARK_RASTER_SCALE` (default 4, accepted range 1–4). At the default scale each terminal cell is backed by `4 × 8` raster samples because one block glyph contains an upper and a lower half-cell.

A half-cell is occupied only when at least two samples are distinguishable from the transparent or detected opaque SVG background. Thus the default minimum retained coverage is

```text
2 / 4^2 = 12.5 percent of one half-cell
```

A one-sample speck is rejected, while a one-sample-wide horizontal, vertical, or diagonal stroke crossing the sample region contributes several samples and remains visible. Lowering `PTYMARK_RASTER_SCALE` is an explicit compatibility trade-off; scale 1 uses the single available sample and therefore cannot provide the same speck rejection.

## Fragmentation guard and fallback

After half-cell reduction, Ptymark measures terminal-cell adjacency with an eight-neighbor grid. If at least ten occupied cells are isolated and isolated cells exceed 60 percent of all occupied cells, the result is classified as a misleading mosaic rather than a successful presentation. The managed presenter exits with a stable failure instead of printing it.

That failure reuses the existing pipeline boundary:

- normal non-strict rendering restores the complete exact source block, which is always visible text;
- `--strict` reports the presentation failure;
- `--source` preserves complete detected source without starting external engines;
- `--safe` preserves all input bytes without semantic rendering.

The presenter does not synthesize a second source representation and does not alter the exact-source / safe fallback owned by the pipeline.

## Color and terminal transport

The terminal-background-safe contract from [presentation-color.md](presentation-color.md) is unchanged. Stroke aggregation happens before color selection; the resulting occupied cells still use terminal-default colors or the existing WCAG 4.5:1 foreground correction. No RGB background is emitted.

`TERM=xterm-256color`, `TERM=tmux-256color`, dark/light appearance hints, SSH, redirection, and tmux do not enter the geometry equations. Given the same SVG, width, and explicit presenter policy, normal PTY and tmux receive identical glyph geometry. Transport-specific rendering and tmux lifecycle remain outside this presenter issue.

## Validation boundary

Focused regressions cover fitted width, cell-aspect compensation, bounded policy parsing, supersampled thin strokes, fraction and superscript structure, fragmentation rejection, dark/light geometry, and `xterm`/`tmux` byte geometry. The managed-bundle smoke renders `E = mc^2`, a fraction, a superscript, and an integral with real MathJax at an 80-column maximum and verifies Japanese text adjacent to a math block remains exact.

Human perception still depends on the installed terminal font and its measured cell dimensions. A real-terminal verification should set `PTYMARK_CELL_ASPECT_RATIO` to the measured cell height divided by cell width when the terminal differs materially from the default 2.0.
