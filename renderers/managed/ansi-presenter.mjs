import fs from 'node:fs/promises';
import process from 'node:process';
import puppeteer from 'puppeteer';

function parseArguments(arguments_) {
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

function blend(channel, alpha, background) {
  return Math.round(channel * alpha + background * (1 - alpha));
}

function pixel(data, offset, background) {
  const alpha = data[offset + 3] / 255;
  if (alpha <= 0.02) return null;
  return [
    blend(data[offset], alpha, background),
    blend(data[offset + 1], alpha, background),
    blend(data[offset + 2], alpha, background),
  ];
}

function luminance(rgb) {
  if (rgb === null) return 0;
  return 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
}

function monochromeCell(top, bottom) {
  const upper = luminance(top) >= 128;
  const lower = luminance(bottom) >= 128;
  if (upper && lower) return '█';
  if (upper) return '▀';
  if (lower) return '▄';
  return ' ';
}

function renderAnsi({ pixels, width, height, color, background }) {
  const lines = [];
  for (let y = 0; y < height; y += 2) {
    let line = '';
    for (let x = 0; x < width; x += 1) {
      const topOffset = (y * width + x) * 4;
      const bottomY = Math.min(y + 1, height - 1);
      const bottomOffset = (bottomY * width + x) * 4;
      const top = pixel(pixels, topOffset, background);
      const bottom = pixel(pixels, bottomOffset, background);

      if (!color) {
        line += monochromeCell(top, bottom);
        continue;
      }
      if (top === null && bottom === null) {
        line += '\x1b[0m ';
        continue;
      }
      const foreground = top ?? [background, background, background];
      const backdrop = bottom ?? [background, background, background];
      line += `\x1b[38;2;${foreground[0]};${foreground[1]};${foreground[2]}m`;
      line += `\x1b[48;2;${backdrop[0]};${backdrop[1]};${backdrop[2]}m▀`;
    }
    lines.push(`${line}\x1b[0m`);
  }
  return `${lines.join('\n')}\n`;
}

async function rasterize(svg, columns, executablePath) {
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

try {
  const options = parseArguments(process.argv.slice(2));
  const svg = await fs.readFile(options.inputPath, 'utf8');
  if (!svg.includes('<svg')) throw new Error('input does not contain an SVG element');
  const raster = await rasterize(
    svg,
    options.columns,
    process.env.PUPPETEER_EXECUTABLE_PATH,
  );
  const appearance = (process.env.PTYMARK_APPEARANCE || 'dark').toLowerCase();
  const background = appearance === 'light' ? 255 : 0;
  process.stdout.write(renderAnsi({ ...raster, color: options.color, background }));
} catch (error) {
  console.error(`ptymark managed presenter: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
