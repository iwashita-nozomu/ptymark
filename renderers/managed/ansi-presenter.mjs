import fs from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import process from 'node:process';

const ALPHA_THRESHOLD = 0.02;
const OPAQUE_THRESHOLD = 0.98;
const CORNER_COLOR_TOLERANCE = 8;
const BACKGROUND_DISTANCE_THRESHOLD = 0.03;
const MIN_CONTRAST_RATIO = 4.5;
const UNKNOWN_BACKGROUND_FOREGROUND = Object.freeze([117, 117, 117]);

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

export function renderAnsi({ pixels, width, height, color, appearance }) {
  const rasterBackground = estimateRasterBackground(pixels, width, height);
  const lines = [];
  let occupiedCells = 0;
  let visibleCells = 0;

  for (let y = 0; y < height; y += 2) {
    let line = '';
    let activeColor = null;

    for (let x = 0; x < width; x += 1) {
      const topOffset = (y * width + x) * 4;
      const bottomOffset = ((y + 1) * width + x) * 4;
      const top = foregroundPixel(pixels, topOffset, rasterBackground);
      const bottom = y + 1 < height
        ? foregroundPixel(pixels, bottomOffset, rasterBackground)
        : null;
      const glyph = cellGlyph(top, bottom);

      if (glyph === ' ') {
        line += glyph;
        continue;
      }
      occupiedCells += 1;

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

  if (occupiedCells === 0) {
    throw new Error('raster contains no foreground terminal cells');
  }
  if (visibleCells === 0) {
    throw new Error('raster contains only zero-contrast terminal cells');
  }
  return `${lines.join('\n')}\n`;
}

async function rasterize(svg, columns, executablePath) {
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
    return await page.evaluate(async ({ dataUrl: url, columns: targetColumns }) => {
      const image = new Image();
      image.decoding = 'sync';
      image.src = url;
      await new Promise((resolve, reject) => {
        image.onload = resolve;
        image.onerror = () => reject(new Error('browser could not decode SVG'));
      });

      const sourceWidth = Math.max(1, image.naturalWidth || image.width || 800);
      const sourceHeight = Math.max(1, image.naturalHeight || image.height || 600);
      const width = targetColumns;
      const height = Math.max(2, Math.min(1024, Math.round(width * sourceHeight / sourceWidth)));
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
    }, { dataUrl, columns });
  } finally {
    await browser.close();
  }
}

async function main() {
  try {
    const options = parseArguments(process.argv.slice(2));
    const svg = await fs.readFile(options.inputPath, 'utf8');
    if (!svg.includes('<svg')) throw new Error('input does not contain an SVG element');
    const raster = await rasterize(
      svg,
      options.columns,
      process.env.PUPPETEER_EXECUTABLE_PATH,
    );
    process.stdout.write(renderAnsi({
      ...raster,
      color: options.color,
      appearance: process.env.PTYMARK_APPEARANCE,
    }));
  } catch (error) {
    console.error(`ptymark managed presenter: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
