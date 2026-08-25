import assert from 'node:assert/strict';
import test from 'node:test';

import {
  contrastRatio,
  parseArguments,
  renderAnsi,
  renderPresentation,
  resolveRasterDimensions,
  resolveScalePolicy,
} from '../renderers/managed/ansi-presenter.mjs';

const BLACK = [0, 0, 0, 255];
const WHITE = [255, 255, 255, 255];
const TRANSPARENT = [0, 0, 0, 0];

function raster(...pixels) {
  return {
    width: pixels.length / 2,
    height: 2,
    pixels: [
      ...pixels.slice(0, pixels.length / 2).flat(),
      ...pixels.slice(pixels.length / 2).flat(),
    ],
  };
}

function highResolutionRaster(pattern, mode = 'stroke', rasterScale = 4) {
  const outputWidth = Math.max(...pattern.map((row) => row.length));
  const outputHalfRows = pattern.length;
  const width = outputWidth * rasterScale;
  const height = outputHalfRows * rasterScale;
  const pixels = new Array(width * height * 4).fill(0);
  const setPixel = (x, y, pixel = BLACK) => {
    const offset = (y * width + x) * 4;
    pixels.splice(offset, 4, ...pixel);
  };

  for (let y = 0; y < outputHalfRows; y += 1) {
    for (let x = 0; x < outputWidth; x += 1) {
      if ((pattern[y][x] || '.') !== '#') continue;
      const startX = x * rasterScale;
      const startY = y * rasterScale;
      if (mode === 'fill') {
        for (let offsetY = 0; offsetY < rasterScale; offsetY += 1) {
          for (let offsetX = 0; offsetX < rasterScale; offsetX += 1) {
            setPixel(startX + offsetX, startY + offsetY);
          }
        }
      } else if (mode === 'speck') {
        setPixel(
          startX + Math.floor(rasterScale / 2),
          startY + Math.floor(rasterScale / 2),
        );
      } else {
        const center = Math.floor(rasterScale / 2);
        for (let offset = 0; offset < rasterScale; offset += 1) {
          setPixel(startX + offset, startY + center);
          setPixel(startX + center, startY + offset);
        }
      }
    }
  }
  return { width, height, pixels, rasterScale };
}

function foregrounds(output) {
  return [...output.matchAll(/\x1b\[38;2;(\d+);(\d+);(\d+)m/g)]
    .map((match) => match.slice(1).map(Number));
}

function geometry(output) {
  return output.replace(/\x1b\[[0-9;]*m/g, '');
}

test('no-color symbols use terminal defaults and preserve black ink shape', () => {
  const input = raster(TRANSPARENT, BLACK, TRANSPARENT, BLACK);
  const outputs = [];
  const originalTerm = process.env.TERM;
  try {
    for (const term of ['xterm-256color', 'tmux-256color']) {
      process.env.TERM = term;
      outputs.push(renderAnsi({ ...input, color: false, appearance: 'dark' }));
      outputs.push(renderAnsi({ ...input, color: false, appearance: 'light' }));
    }
  } finally {
    if (originalTerm === undefined) delete process.env.TERM;
    else process.env.TERM = originalTerm;
  }

  assert.equal(new Set(outputs).size, 1);
  assert.match(outputs[0], /[█▀▄]/u);
  assert.doesNotMatch(outputs[0], /\x1b\[/);
});

test('opaque raster background is removed before symbol occupancy', () => {
  const input = raster(WHITE, BLACK, WHITE, WHITE, BLACK, WHITE);
  const output = renderAnsi({ ...input, color: false, appearance: 'dark' });

  assert.equal(output, ' █ \n');
});

test('color symbols have contrast on dark, light, and unknown backgrounds', () => {
  const input = raster(TRANSPARENT, BLACK, TRANSPARENT, BLACK);
  const scenarios = [
    ['dark', [[0, 0, 0]]],
    ['light', [[255, 255, 255]]],
    ['unknown', [[0, 0, 0], [255, 255, 255]]],
  ];

  for (const [appearance, backgrounds] of scenarios) {
    const output = renderAnsi({ ...input, color: true, appearance });
    assert.match(output, /[█▀▄]/u);
    assert.doesNotMatch(output, /\x1b\[48;/);
    const colors = foregrounds(output);
    assert.ok(colors.length > 0);
    for (const foreground of colors) {
      for (const background of backgrounds) {
        assert.ok(
          contrastRatio(foreground, background) >= 4.5,
          `${appearance} foreground ${foreground} must contrast with ${background}`,
        );
      }
    }
  }
});

test('foreground correction removes black-on-black and white-on-white cells', () => {
  const darkOutput = renderAnsi({
    ...raster(TRANSPARENT, BLACK, TRANSPARENT, BLACK),
    color: true,
    appearance: 'dark',
  });
  const lightOutput = renderAnsi({
    ...raster(TRANSPARENT, WHITE, TRANSPARENT, WHITE),
    color: true,
    appearance: 'light',
  });

  assert.deepEqual(foregrounds(darkOutput), [[255, 255, 255]]);
  assert.deepEqual(foregrounds(lightOutput), [[0, 0, 0]]);
});

test('all-background rasters are presentation failures', () => {
  for (const input of [
    raster(TRANSPARENT, TRANSPARENT, TRANSPARENT, TRANSPARENT),
    raster(WHITE, WHITE, WHITE, WHITE),
  ]) {
    assert.throws(
      () => renderAnsi({ ...input, color: true, appearance: 'dark' }),
      /no foreground terminal cells/,
    );
  }
});

test('chafa-compatible color arguments remain explicit', () => {
  assert.equal(
    parseArguments(['--colors', 'none', '--size', '40x', 'artifact.svg']).color,
    false,
  );
  assert.equal(
    parseArguments(['--colors', 'full', '--size', '40x', 'artifact.svg']).color,
    true,
  );
});

test('cell aspect and supersampling policy are explicit and bounded', () => {
  assert.deepEqual(resolveScalePolicy({}), { cellAspect: 0.5, rasterScale: 4 });
  assert.deepEqual(
    resolveScalePolicy({ PTYMARK_CELL_ASPECT: '0.6', PTYMARK_RASTER_SCALE: '2' }),
    { cellAspect: 0.6, rasterScale: 2 },
  );
  assert.throws(
    () => resolveScalePolicy({ PTYMARK_CELL_ASPECT: '0.2' }),
    /between 0.25 and 1/,
  );
  assert.throws(
    () => resolveScalePolicy({ PTYMARK_RASTER_SCALE: '8' }),
    /between 2 and 4/,
  );
  assert.throws(
    () => resolveScalePolicy({ PTYMARK_RASTER_SCALE: '2.5' }),
    /must be an integer/,
  );
});

test('raster dimensions compensate terminal cell geometry before bounded supersampling', () => {
  assert.deepEqual(
    resolveRasterDimensions({
      sourceWidth: 160,
      sourceHeight: 40,
      columns: 80,
      cellAspect: 0.5,
      rasterScale: 4,
    }),
    {
      width: 320,
      height: 80,
      outputWidth: 80,
      outputHalfRows: 20,
      cellAspect: 0.5,
      rasterScale: 4,
    },
  );
  assert.equal(
    resolveRasterDimensions({
      sourceWidth: 160,
      sourceHeight: 40,
      columns: 80,
      cellAspect: 0.6,
      rasterScale: 4,
    }).outputHalfRows,
    24,
  );
});

test('supersampled common math strokes retain coverage without isolated-cell noise', () => {
  const equation = highResolutionRaster([
    '###...##....#.#...##.........##',
    '#.....##....###..#..........#.#',
    '##..........###..#............#',
    '#.....##....#.#..#...........#.',
    '###...##....#.#...##.........###',
  ]);
  const fraction = highResolutionRaster([
    '...##...',
    '..#.#...',
    '.######.',
    '...#....',
    '..###...',
  ]);
  const integralAndSuperscript = highResolutionRaster([
    '..##......##',
    '.#........#.',
    '.#........##',
    '.#.........#',
    '..##......###',
  ]);

  const equationPresentation = renderPresentation({
    ...equation,
    color: false,
    appearance: 'dark',
  });
  assert.ok(equationPresentation.metrics.occupiedSubcells >= 25);
  assert.ok(equationPresentation.metrics.isolatedRatio <= 0.1);
  assert.ok(equationPresentation.metrics.maxHorizontalRun >= 3);
  assert.ok(equationPresentation.metrics.maxVerticalRun >= 3);

  const fractionPresentation = renderPresentation({
    ...fraction,
    color: false,
    appearance: 'light',
  });
  assert.ok(fractionPresentation.metrics.maxHorizontalRun >= 6);
  assert.ok(fractionPresentation.metrics.occupiedHalfRows >= 5);

  const integralPresentation = renderPresentation({
    ...integralAndSuperscript,
    color: false,
    appearance: 'dark',
  });
  assert.ok(integralPresentation.metrics.maxVerticalRun >= 3);
  assert.ok(integralPresentation.metrics.occupiedColumns >= 5);
});

test('dark, light, normal PTY, and tmux hints preserve identical geometry', () => {
  const input = highResolutionRaster([
    '###..##..#.#',
    '#....##..###',
    '##.......###',
    '#....##..#.#',
    '###..##..#.#',
  ]);
  const outputs = [];
  const originalTerm = process.env.TERM;
  try {
    for (const term of ['xterm-256color', 'tmux-256color']) {
      process.env.TERM = term;
      for (const appearance of ['dark', 'light']) {
        outputs.push(renderAnsi({ ...input, color: true, appearance }));
      }
    }
  } finally {
    if (originalTerm === undefined) delete process.env.TERM;
    else process.env.TERM = originalTerm;
  }
  assert.equal(new Set(outputs.map(geometry)).size, 1);
});

test('subcell coverage removes isolated high-resolution specks', () => {
  const specks = highResolutionRaster([
    '.........',
    '.#.#.#.#.',
    '.........',
    '..#.#.#..',
    '.........',
    '.#.#.#.#.',
    '.........',
  ], 'speck');
  assert.throws(
    () => renderAnsi({ ...specks, color: false, appearance: 'dark' }),
    /no foreground terminal cells/,
  );
});

test('fragmented block mosaics fail closed for exact-source fallback', () => {
  const fragments = highResolutionRaster([
    '.........',
    '.#.#.#.#.',
    '.........',
    '..#.#.#..',
    '.........',
    '.#.#.#.#.',
    '.........',
  ], 'fill');
  assert.throws(
    () => renderAnsi({ ...fragments, color: false, appearance: 'dark' }),
    /too fragmented for legible presentation/,
  );
});
