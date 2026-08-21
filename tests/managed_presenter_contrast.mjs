import assert from 'node:assert/strict';
import test from 'node:test';

import {
  contrastRatio,
  parseArguments,
  renderAnsi,
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

function foregrounds(output) {
  return [...output.matchAll(/\x1b\[38;2;(\d+);(\d+);(\d+)m/g)]
    .map((match) => match.slice(1).map(Number));
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
