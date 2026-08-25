import fs from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import process from 'node:process';

const ALPHA_THRESHOLD = 0.02;
const OPAQUE_THRESHOLD = 0.98;
const CORNER_COLOR_TOLERANCE = 8;
const BACKGROUND_DISTANCE_THRESHOLD = 0.03;
const MIN_CONTRAST_RATIO = 4.5;
const UNKNOWN_BACKGROUND_FOREGROUND = Object.freeze([117, 117, 117]);

const DEFAULT_CELL_ASPECT = 0.5;
const MIN_CELL_ASPECT = 0.25;
const MAX_CELL_ASPECT = 1;
const DEFAULT_RASTER_SCALE = 4;
const MIN_RASTER_SCALE = 2;
const MAX_RASTER_SCALE = 4;
const MAX_OUTPUT_HALF_ROWS = 1024;
const MIN_STROKE_COVERAGE = 0.1;
const MIN_STRUCTURE_SUBCELLS = 8;
const MAX_ISOLATED_SUBCELL_RATIO = 0.3;

export function parseArguments(arguments_) {
  let columns = 80;
  let color = true;
  let inputPath;

  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    switch (argument) {
      case '--format':
      case '--probe':
      case '--polite':
      case '--relative':
      case '--animate':
        index += 1;
        break;
      case '--colors': {
        const value = arguments_[index + 1];
        if (value === undefined) throw new Error('missing value after --colors');
        color = value !== 'none';
        index += 1;
        break;
      }
      case '--size': {
        const value = arguments_[index + 1];
        if (value === undefined) throw new Error('missing value after --size');
        const match = /^(\d+)x/.exec(value);
        if (!match) throw new Error(`unsupported --size value: ${value}`);
        columns = Number.parseInt(match[1], 10);
        index += 1;
        break;
      }
      default:
        if (argument.startsWith('-')) {
          throw new Error(`unsupported presenter option: ${argument}`);
        }
        inputPath = argument;
        break;
    }
  }

  if (!inputPath) throw new Error('missing SVG input path');
  if (!Number.isSafeInteger(columns) || columns < 1 || columns > 512) {
    throw new Error('terminal width must be between 1 and 512 columns');
  }
  return { columns, color, inputPath };
}

function boundedNumber(name, rawValue, defaultValue, minimum, maximum, integer) {
  if (rawValue === undefined || String(rawValue).trim() === '') return defaultValue;
  const text = String(rawValue).trim();
  if (integer && !/^\d+$/.test(text)) {
    throw new Error(`${name} must be an integer between ${minimum} and ${maximum}`);
  }
  const value = Number(text);
  if (!Number.isFinite(value) || (integer && !Number.isSafeInteger(value))
      || value < minimum || value > maximum) {
    throw new Error(`${name} must be between ${minimum} and ${maximum}`);
  }
  return value;
}

export function resolveScalePolicy(environment = {}) {
  return Object.freeze({
    cellAspect: boundedNumber(
      'PTYMARK_CELL_ASPECT',
      environment.PTYMARK_CELL_ASPECT,
      DEFAULT_CELL_ASPECT,
      MIN_CELL_ASPECT,
      MAX_CELL_ASPECT,
      false,
    ),
    rasterScale: boundedNumber(
      'PTYMARK_RASTER_SCALE',
      environment.PTYMARK_RASTER_SCALE,
      DEFAULT_RASTER_SCALE,
      MIN_RASTER_SCALE,
      MAX_RASTER_SCALE,
      true,
    ),
  });
}

export function resolveRasterDimensions({
  sourceWidth,
  sourceHeight,
  columns,
  cellAspect,
  rasterScale,
}) {
  if (!Number.isFinite(sourceWidth) || sourceWidth <= 0
      || !Number.isFinite(sourceHeight) || sourceHeight <= 0) {
    throw new Error('SVG dimensions must be positive finite numbers');
  }
  if (!Number.isSafeInteger(columns) || columns < 1 || columns > 512) {
    throw new Error('terminal width must be between 1 and 512 columns');
  }
  if (!Number.isFinite(cellAspect)
      || cellAspect < MIN_CELL_ASPECT || cellAspect > MAX_CELL_ASPECT) {
    throw new Error(`cell aspect must be between ${MIN_CELL_ASPECT} and ${MAX_CELL_ASPECT}`);
  }
  if (!Number.isSafeInteger(rasterScale)
      || rasterScale < MIN_RASTER_SCALE || rasterScale > MAX_RASTER_SCALE) {
    throw new Error(`raster scale must be between ${MIN_RASTER_SCALE} and ${MAX_RASTER_SCALE}`);
  }

  // Each terminal row carries two vertical half-cells. For terminal cell
  // aspect a = cell_width / cell_height, source aspect is preserved by
  // H_half = 2 * columns * a * source_height / source_width.
  const outputHalfRows = Math.max(
    2,
    Math.min(
      MAX_OUTPUT_HALF_ROWS,
      Math.round(2 * columns * cellAspect * sourceHeight / sourceWidth),
    ),
  );
  return Object.freeze({
    width: columns * rasterScale,
    height: outputHalfRows * rasterScale,
    outputWidth: columns,
    outputHalfRows,
    cellAspect,
    rasterScale,
  });
}

function rawPixel(data, offset) {
  const alpha = data[offset + 3] / 255;
  if (alpha <= ALPHA_THRESHOLD) return null;
  return {
    alpha,
    rgb: [data[offset], data[offset + 1], data[offset + 2]],
  };
}

function channelDistance(left, right) {
  return Math.max(
    Math.abs(left[0] - right[0]),
    Math.abs(left[1] - right[1]),
    Math.abs(left[2] - right[2]),
  );
}

function estimateRasterBackground(pixels, width, height) {
  const offsets = [
    0,
    (width - 1) * 4,
    (height - 1) * width * 4,
    ((height * width) - 1) * 4,
  ];
  const corners = offsets
    .map((offset) => rawPixel(pixels, offset))
    .filter((sample) => sample !== null && sample.alpha >= OPAQUE_THRESHOLD);

  for (const candidate of corners) {
    const inliers = corners.filter(
      (sample) => channelDistance(sample.rgb, candidate.rgb) <= CORNER_COLOR_TOLERANCE,
    );
    if (inliers.length < 3) continue;
    return [0, 1, 2].map((channel) => Math.round(
      inliers.reduce((total, sample) => total + sample.rgb[channel], 0)
        / inliers.length,
    ));
  }
  return null;
}

function linearChannel(channel) {
  const value = channel / 255;
  return value <= 0.04045
    ? value / 12.92
    : ((value + 0.055) / 1.055) ** 2.4;
}

function linearRgbDistance(left, right) {
  return Math.sqrt([0, 1, 2].reduce((distance, channel) => {
    const difference = linearChannel(left[channel]) - linearChannel(right[channel]);
    return distance + (difference * difference);
  }, 0));
}

function blend(channel, alpha, background) {
  return Math.round(channel * alpha + background * (1 - alpha));
}

function foregroundPixel(data, offset, background) {
  const sample = rawPixel(data, offset);
  if (sample === null) return null;
  if (background === null) return sample;

  const rgb = [0, 1, 2].map(
    (channel) => blend(sample.rgb[channel], sample.alpha, background[channel]),
  );
  if (linearRgbDistance(rgb, background) < BACKGROUND_DISTANCE_THRESHOLD) {
    return null;
  }
  return { alpha: sample.alpha, rgb };
}

function aggregateSubcell({ pixels, width, x, y, rasterScale, rasterBackground }) {
  const weightedRgb = [0, 0, 0];
  let ink = 0;

  for (let offsetY = 0; offsetY < rasterScale; offsetY += 1) {
    for (let offsetX = 0; offsetX < rasterScale; offsetX += 1) {
      const sourceX = x * rasterScale + offsetX;
      const sourceY = y * rasterScale + offsetY;
      const offset = (sourceY * width + sourceX) * 4;
      const sample = foregroundPixel(pixels, offset, rasterBackground);
      if (sample === null) continue;
      ink += sample.alpha;
      for (let channel = 0; channel < 3; channel += 1) {
        weightedRgb[channel] += sample.rgb[channel] * sample.alpha;
      }
    }
  }

  const coverage = ink / (rasterScale * rasterScale);
  if (coverage < MIN_STROKE_COVERAGE) return null;
  return {
    alpha: Math.min(1, coverage),
    coverage,
    rgb: weightedRgb.map((value) => Math.round(value / ink)),
  };
}

function terminalSubcells({ pixels, width, height, rasterScale }) {
  if (!Number.isSafeInteger(rasterScale) || rasterScale < 1) {
    throw new Error('raster scale must be a positive integer');
  }
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height)
      || width < rasterScale || height < rasterScale
      || width % rasterScale !== 0 || height % rasterScale !== 0) {
    throw new Error('raster dimensions must be positive multiples of raster scale');
  }
  if (!Array.isArray(pixels) && !ArrayBuffer.isView(pixels)) {
    throw new Error('raster pixels must be an array-like RGBA buffer');
  }
  if (pixels.length !== width * height * 4) {
    throw new Error('raster pixel buffer length does not match dimensions');
  }

  const outputWidth = width / rasterScale;
  const outputHalfRows = height / rasterScale;
  const rasterBackground = estimateRasterBackground(pixels, width, height);
  const subcells = [];
  for (let y = 0; y < outputHalfRows; y += 1) {
    for (let x = 0; x < outputWidth; x += 1) {
      subcells.push(aggregateSubcell({
        pixels,
        width,
        x,
        y,
        rasterScale,
        rasterBackground,
      }));
    }
  }
  return { subcells, outputWidth, outputHalfRows };
}

export function presentationMetrics(subcells, width, height) {
  const occupied = (x, y) => x >= 0 && x < width && y >= 0 && y < height
    && subcells[y * width + x] !== null;
  let occupiedSubcells = 0;
  let isolatedSubcells = 0;
  let occupiedColumns = 0;
  let occupiedHalfRows = 0;
  let maxHorizontalRun = 0;
  let maxVerticalRun = 0;

  for (let y = 0; y < height; y += 1) {
    let rowOccupied = false;
    let run = 0;
    for (let x = 0; x < width; x += 1) {
      if (!occupied(x, y)) {
        run = 0;
        continue;
      }
      occupiedSubcells += 1;
      rowOccupied = true;
      run += 1;
      maxHorizontalRun = Math.max(maxHorizontalRun, run);

      let hasNeighbor = false;
      for (let neighborY = y - 1; neighborY <= y + 1; neighborY += 1) {
        for (let neighborX = x - 1; neighborX <= x + 1; neighborX += 1) {
          if ((neighborX !== x || neighborY !== y) && occupied(neighborX, neighborY)) {
            hasNeighbor = true;
          }
        }
      }
      if (!hasNeighbor) isolatedSubcells += 1;
    }
    if (rowOccupied) occupiedHalfRows += 1;
  }

  for (let x = 0; x < width; x += 1) {
    let columnOccupied = false;
    let run = 0;
    for (let y = 0; y < height; y += 1) {
      if (!occupied(x, y)) {
        run = 0;
        continue;
      }
      columnOccupied = true;
      run += 1;
      maxVerticalRun = Math.max(maxVerticalRun, run);
    }
    if (columnOccupied) occupiedColumns += 1;
  }

  return Object.freeze({
    occupiedSubcells,
    isolatedSubcells,
    isolatedRatio: occupiedSubcells === 0 ? 0 : isolatedSubcells / occupiedSubcells,
    occupiedColumns,
    occupiedHalfRows,
    maxHorizontalRun,
    maxVerticalRun,
  });
}

function assertLegible(metrics) {
  if (metrics.occupiedSubcells === 0) {
    throw new Error('raster contains no foreground terminal cells');
  }
  if (metrics.occupiedSubcells >= MIN_STRUCTURE_SUBCELLS
      && metrics.isolatedRatio > MAX_ISOLATED_SUBCELL_RATIO) {
    throw new Error('raster terminal-cell coverage is too fragmented for legible presentation');
  }
}

function cellGlyph(top, bottom) {
  if (top !== null && bottom !== null) return '█';
  if (top !== null) return '▀';
  if (bottom !== null) return '▄';
  return ' ';
}

function representativeColor(top, bottom) {
  const samples = [top, bottom].filter((sample) => sample !== null);
  const totalAlpha = samples.reduce((total, sample) => total + sample.alpha, 0);
  return [0, 1, 2].map((channel) => Math.round(
    samples.reduce(
      (total, sample) => total + sample.rgb[channel] * sample.alpha,
      0,
    ) / totalAlpha,
  ));
}

export function relativeLuminance(rgb) {
  return 0.2126 * linearChannel(rgb[0])
    + 0.7152 * linearChannel(rgb[1])
    + 0.0722 * linearChannel(rgb[2]);
}

export function contrastRatio(left, right) {
  const leftLuminance = relativeLuminance(left);
  const rightLuminance = relativeLuminance(right);
  const lighter = Math.max(leftLuminance, rightLuminance);
  const darker = Math.min(leftLuminance, rightLuminance);
  return (lighter + 0.05) / (darker + 0.05);
}

function normalizeAppearance(value) {
  const appearance = String(value || '').trim().toLowerCase();
  return appearance === 'dark' || appearance === 'light' ? appearance : 'unknown';
}

export function contrastSafeForeground(rgb, appearance) {
  switch (normalizeAppearance(appearance)) {
    case 'dark':
      return contrastRatio(rgb, [0, 0, 0]) >= MIN_CONTRAST_RATIO
        ? rgb
        : [255, 255, 255];
    case 'light':
      return contrastRatio(rgb, [255, 255, 255]) >= MIN_CONTRAST_RATIO
        ? rgb
        : [0, 0, 0];
    default:
      // #757575 lies near the equal-contrast point and clears 4.5:1
      // against both ideal black and ideal white.
      return [...UNKNOWN_BACKGROUND_FOREGROUND];
  }
}

function terminalBackgrounds(appearance) {
  switch (normalizeAppearance(appearance)) {
    case 'dark':
      return [[0, 0, 0]];
    case 'light':
      return [[255, 255, 255]];
    default:
      return [[0, 0, 0], [255, 255, 255]];
  }
}

export function renderPresentation({
  pixels,
  width,
  height,
  color,
  appearance,
  rasterScale = 1,
}) {
  const { subcells, outputWidth, outputHalfRows } = terminalSubcells({
    pixels,
    width,
    height,
    rasterScale,
  });
  const metrics = presentationMetrics(subcells, outputWidth, outputHalfRows);
  assertLegible(metrics);

  const lines = [];
  let visibleCells = 0;

  for (let y = 0; y < outputHalfRows; y += 2) {
    let line = '';
    let activeColor = null;

    for (let x = 0; x < outputWidth; x += 1) {
      const top = subcells[y * outputWidth + x];
      const bottom = y + 1 < outputHalfRows
        ? subcells[(y + 1) * outputWidth + x]
        : null;
      const glyph = cellGlyph(top, bottom);

      if (glyph === ' ') {
        line += glyph;
        continue;
      }

      if (!color) {
        visibleCells += 1;
        line += glyph;
        continue;
      }

      const foreground = contrastSafeForeground(
        representativeColor(top, bottom),
        appearance,
      );
      if (terminalBackgrounds(appearance).every(
        (background) => contrastRatio(foreground, background) >= MIN_CONTRAST_RATIO,
      )) {
        visibleCells += 1;
      }
      const key = foreground.join(';');
      if (activeColor !== key) {
        line += `\x1b[38;2;${key}m`;
        activeColor = key;
      }
      line += glyph;
    }

    if (activeColor !== null) line += '\x1b[39m';
    lines.push(line);
  }

  if (visibleCells === 0) {
    throw new Error('raster contains only zero-contrast terminal cells');
  }
  return Object.freeze({ output: `${lines.join('\n')}\n`, metrics });
}

export function renderAnsi(options) {
  return renderPresentation(options).output;
}

async function loadImage(page, dataUrl) {
  return page.evaluate(async (url) => {
    const image = new Image();
    image.decoding = 'sync';
    image.src = url;
    await new Promise((resolve, reject) => {
      image.onload = resolve;
      image.onerror = () => reject(new Error('browser could not decode SVG'));
    });
    return {
      width: Math.max(1, image.naturalWidth || image.width || 800),
      height: Math.max(1, image.naturalHeight || image.height || 600),
    };
  }, dataUrl);
}

async function rasterize(svg, columns, executablePath, scalePolicy) {
  const { default: puppeteer } = await import('puppeteer');
  const browser = await puppeteer.launch({
    headless: true,
    executablePath: executablePath || undefined,
    args: process.env.PTYMARK_BROWSER_NO_SANDBOX === '1'
      ? ['--no-sandbox', '--disable-setuid-sandbox']
      : [],
  });
  try {
    const page = await browser.newPage();
    const dataUrl = `data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}`;
    const source = await loadImage(page, dataUrl);
    const dimensions = resolveRasterDimensions({
      sourceWidth: source.width,
      sourceHeight: source.height,
      columns,
      ...scalePolicy,
    });
    const raster = await page.evaluate(async ({ url, width, height }) => {
      const image = new Image();
      image.decoding = 'sync';
      image.src = url;
      await new Promise((resolve, reject) => {
        image.onload = resolve;
        image.onerror = () => reject(new Error('browser could not decode SVG'));
      });

      const canvas = document.createElement('canvas');
      canvas.width = width;
      canvas.height = height;
      const context = canvas.getContext('2d', { willReadFrequently: true });
      context.clearRect(0, 0, width, height);
      context.drawImage(image, 0, 0, width, height);
      return {
        width,
        height,
        pixels: Array.from(context.getImageData(0, 0, width, height).data),
      };
    }, { url: dataUrl, width: dimensions.width, height: dimensions.height });
    return { ...raster, ...dimensions };
  } finally {
    await browser.close();
  }
}

function diagnosticsEnabled(environment) {
  return String(environment.PTYMARK_PRESENTER_DIAGNOSTICS || '').trim() === '1';
}

async function main() {
  try {
    const options = parseArguments(process.argv.slice(2));
    const scalePolicy = resolveScalePolicy(process.env);
    const svg = await fs.readFile(options.inputPath, 'utf8');
    if (!svg.includes('<svg')) throw new Error('input does not contain an SVG element');
    const raster = await rasterize(
      svg,
      options.columns,
      process.env.PUPPETEER_EXECUTABLE_PATH,
      scalePolicy,
    );
    const presentation = renderPresentation({
      ...raster,
      color: options.color,
      appearance: process.env.PTYMARK_APPEARANCE,
    });
    if (diagnosticsEnabled(process.env)) {
      console.error(
        'ptymark managed presenter:'
          + ` terminal=${raster.outputWidth}x${Math.ceil(raster.outputHalfRows / 2)}`
          + ` raster=${raster.width}x${raster.height}`
          + ` cell_aspect=${raster.cellAspect}`
          + ` raster_scale=${raster.rasterScale}`
          + ` occupied_half_cells=${presentation.metrics.occupiedSubcells}`
          + ` isolated_ratio=${presentation.metrics.isolatedRatio.toFixed(3)}`,
      );
    }
    process.stdout.write(presentation.output);
  } catch (error) {
    console.error(`ptymark managed presenter: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
