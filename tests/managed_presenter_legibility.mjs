import assert from 'node:assert/strict';
import test from 'node:test';

import {
  fragmentationMetrics,
  rasterDimensions,
  renderAnsi,
  resolveRasterPolicy,
} from '../renderers/managed/ansi-presenter.mjs';

const BLACK = [0, 0, 0, 255];
const TRANSPARENT = [0, 0, 0, 0];
const ANSI = /\x1b\[[0-9;]*m/g;

function emptyRaster(columns, rows, sampleScale = 4) {
  const width = columns * sampleScale;
  const height = rows * 2 * sampleScale;
  return {
    columns,
    rows,
    sampleScale,
    width,
    height,
    pixels: Array.from({ length: width * height }, () => TRANSPARENT).flat(),
  };
}

function setPixel(raster, x, y, rgba = BLACK) {
  const offset = (y * raster.width + x) * 4;
  raster.pixels.splice(offset, 4, ...rgba);
}

function paintHalfCell(raster, column, halfRow, points) {
  const startX = column * raster.sampleScale;
  const startY = halfRow * raster.sampleScale;
  for (const [dx, dy] of points) setPixel(raster, startX + dx, startY + dy);
}

function horizontalStroke(scale) {
  const y = Math.floor(scale / 2);
  return Array.from({ length: scale }, (_, x) => [x, y]);
}

function geometry(output) {
  return output.replaceAll(ANSI, '');
}

test('raster policy is explicit and bounded', () => {
  assert.deepEqual(resolveRasterPolicy({}), {
    cellAspectRatio: 2,
    sourcePixelsPerCell: 8,
    rasterScale: 4,
  });
  assert.deepEqual(resolveRasterPolicy({
    PTYMARK_CELL_ASPECT_RATIO: '1.75',
    PTYMARK_SOURCE_PIXELS_PER_CELL: '6',
    PTYMARK_RASTER_SCALE: '2',
  }), {
    cellAspectRatio: 1.75,
    sourcePixelsPerCell: 6,
    rasterScale: 2,
  });

  for (const environment of [
    { PTYMARK_CELL_ASPECT_RATIO: '0.9' },
    { PTYMARK_CELL_ASPECT_RATIO: '3.1' },
    { PTYMARK_SOURCE_PIXELS_PER_CELL: '3.9' },
    { PTYMARK_SOURCE_PIXELS_PER_CELL: '16.1' },
    { PTYMARK_RASTER_SCALE: '0' },
    { PTYMARK_RASTER_SCALE: '5' },
    { PTYMARK_RASTER_SCALE: '2.5' },
  ]) {
    assert.throws(() => resolveRasterPolicy(environment), /must be/u);
  }
});

test('terminal rows compensate the configured physical cell aspect ratio', () => {
  const dimensions = rasterDimensions({
    sourceWidth: 400,
    sourceHeight: 100,
    columns: 80,
    cellAspectRatio: 2,
    sourcePixelsPerCell: 8,
    rasterScale: 4,
  });
  assert.deepEqual(dimensions, {
    requestedColumns: 80,
    columns: 50,
    rows: 7,
    sampleScale: 4,
    width: 200,
    height: 56,
    cellAspectRatio: 2,
    sourcePixelsPerCell: 8,
  });
  assert.ok(
    Math.abs(dimensions.rows * dimensions.cellAspectRatio / dimensions.columns - 0.25)
      <= dimensions.cellAspectRatio / dimensions.columns,
  );

  const narrowerCells = rasterDimensions({
    sourceWidth: 400,
    sourceHeight: 100,
    columns: 80,
    cellAspectRatio: 1.5,
    sourcePixelsPerCell: 4,
    rasterScale: 2,
  });
  assert.equal(narrowerCells.columns, 80);
  assert.equal(narrowerCells.rows, 14);
  assert.ok(
    Math.abs(narrowerCells.rows * 1.5 / 80 - 0.25) <= 1.5 / 80,
    'rounding error is bounded by one terminal row',
  );
});

test('four-times sampling preserves one-sample-wide strokes across a half-cell', () => {
  const input = emptyRaster(5, 1);
  for (let column = 0; column < input.columns; column += 1) {
    paintHalfCell(input, column, 0, horizontalStroke(input.sampleScale));
  }

  assert.equal(
    renderAnsi({ ...input, color: false, appearance: 'dark' }),
    '▀▀▀▀▀\n',
  );
});

test('a single supersample speck is rejected instead of becoming a block mosaic', () => {
  const input = emptyRaster(1, 1);
  paintHalfCell(input, 0, 0, [[1, 1]]);

  assert.throws(
    () => renderAnsi({ ...input, color: false, appearance: 'dark' }),
    /no foreground terminal cells/u,
  );
});

test('fraction bars and separated superscripts retain two-dimensional structure', () => {
  const input = emptyRaster(9, 3);
  const stroke = horizontalStroke(input.sampleScale);

  for (const column of [2, 3, 4]) paintHalfCell(input, column, 0, stroke);
  for (let column = 1; column <= 5; column += 1) paintHalfCell(input, column, 2, stroke);
  for (const column of [2, 3, 4]) paintHalfCell(input, column, 5, stroke);
  for (let halfRow = 2; halfRow <= 5; halfRow += 1) {
    paintHalfCell(input, 7, halfRow, stroke);
  }
  paintHalfCell(input, 8, 0, stroke);

  const lines = geometry(renderAnsi({ ...input, color: true, appearance: 'dark' }))
    .trimEnd()
    .split('\n');
  assert.equal(lines.length, 3);
  assert.match(lines[0], /▀▀▀.*▀/u);
  assert.match(lines[1], /[▄█▀]{5}/u);
  assert.match(lines[2], /[▄█▀]{3}/u);
});

test('fragmentation metric rejects many isolated cells but permits compact strokes', () => {
  const fragmented = [
    '█  █  █  █  █',
    '              ',
    '              ',
    '█  █  █  █  █',
  ];
  assert.deepEqual(fragmentationMetrics(fragmented), {
    occupied: 10,
    isolated: 10,
    isolatedRatio: 1,
  });

  const compact = fragmentationMetrics([' ███ ', '█████', ' ███ ']);
  assert.equal(compact.isolated, 0);
});

test('over-fragmented raster fails closed to the existing source fallback boundary', () => {
  const input = emptyRaster(14, 4);
  const stroke = horizontalStroke(input.sampleScale);
  for (const halfRow of [0, 6]) {
    for (const column of [0, 3, 6, 9, 12]) {
      paintHalfCell(input, column, halfRow, stroke);
    }
  }

  assert.throws(
    () => renderAnsi({ ...input, color: false, appearance: 'dark' }),
    /too fragmented for legible terminal presentation/u,
  );
});

test('dark, light, xterm, and tmux preserve identical terminal geometry', () => {
  const input = emptyRaster(8, 2);
  const stroke = horizontalStroke(input.sampleScale);
  for (let column = 1; column <= 6; column += 1) {
    paintHalfCell(input, column, column % 2, stroke);
    paintHalfCell(input, column, 3, stroke);
  }

  const outputs = [];
  const originalTerm = process.env.TERM;
  try {
    for (const term of ['xterm-256color', 'tmux-256color']) {
      process.env.TERM = term;
      for (const appearance of ['dark', 'light']) {
        outputs.push(geometry(renderAnsi({ ...input, color: true, appearance })));
      }
    }
  } finally {
    if (originalTerm === undefined) delete process.env.TERM;
    else process.env.TERM = originalTerm;
  }
  assert.equal(new Set(outputs).size, 1);
});

test('an eighty-column grid keeps bounded width and continuous stroke coverage', () => {
  const input = emptyRaster(80, 2);
  const stroke = horizontalStroke(input.sampleScale);
  for (let column = 8; column < 72; column += 1) {
    paintHalfCell(input, column, 1, stroke);
  }
  for (let column = 58; column < 65; column += 1) {
    paintHalfCell(input, column, 0, stroke);
  }
  for (let column = 8; column < 72; column += 1) {
    paintHalfCell(input, column, 2, stroke);
  }

  const lines = geometry(renderAnsi({ ...input, color: false, appearance: 'dark' }))
    .trimEnd()
    .split('\n');
  assert.equal(lines.length, 2);
  assert.equal([...lines[0]].length, 80);
  assert.match(lines[0], /[▄█▀]{64}/u);
  assert.equal(fragmentationMetrics(lines).isolated, 0);
});
