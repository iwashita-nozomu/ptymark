import { Buffer } from "node:buffer";
import { once } from "node:events";
import process from "node:process";
import readline from "node:readline";
import { pathToFileURL } from "node:url";

import MathJax from "@mathjax/src/source";
import { renderMermaid } from "@mermaid-js/mermaid-cli";
import katex from "katex";
import puppeteer from "puppeteer";

let browserPromise;
let mathJaxReady;

async function browser() {
  browserPromise ??= puppeteer.launch({
    executablePath: process.env.PUPPETEER_EXECUTABLE_PATH || undefined,
    headless: true,
    args: ["--no-sandbox", "--disable-setuid-sandbox"],
  });
  return await browserPromise;
}

async function mathJax() {
  mathJaxReady ??= MathJax.init({
    loader: { load: ["input/tex", "output/svg"] },
    svg: { fontCache: "local" },
  });
  await mathJaxReady;
  return MathJax;
}

async function renderMathJax(source) {
  const engine = await mathJax();
  const node = engine.tex2svg(source, { display: true });
  return Buffer.from(engine.startup.adaptor.serializeXML(node), "utf8");
}

function renderKatex(source) {
  return Buffer.from(
    katex.renderToString(source, {
      displayMode: true,
      output: "mathml",
      throwOnError: true,
    }),
    "utf8",
  );
}

async function renderDiagram(request) {
  const activeBrowser = await browser();
  const width = Number.isInteger(request.width) ? request.width : 800;
  const height = Number.isInteger(request.height) ? request.height : 600;
  const theme = typeof request.theme === "string" ? request.theme : "default";
  const { data } = await renderMermaid(activeBrowser, request.source, "svg", {
    viewport: { width, height, deviceScaleFactor: 1 },
    backgroundColor: "transparent",
    mermaidConfig: { theme },
  });
  return Buffer.from(data);
}

export async function renderRequest(request) {
  if (!request || typeof request !== "object") {
    throw new Error("request must be an object");
  }
  if (typeof request.source !== "string") {
    throw new Error("request.source must be a string");
  }

  const variant =
    typeof request.engine === "string" && request.engine.length > 0
      ? request.engine
      : undefined;
  let bytes;
  let mediaType;
  let engine;
  switch (request.kind) {
    case "math":
      if (variant === "katex") {
        bytes = renderKatex(request.source);
        mediaType = "application/mathml+xml";
        engine = "katex/0.17.0";
      } else {
        bytes = await renderMathJax(request.source);
        mediaType = "image/svg+xml";
        engine = "mathjax/4.1.3";
      }
      break;
    case "mermaid":
      bytes = await renderDiagram(request);
      mediaType = "image/svg+xml";
      engine = "mermaid-cli/11.16.0";
      break;
    default:
      throw new Error(`unsupported renderer kind: ${request.kind}`);
  }

  return {
    id: request.id ?? null,
    ok: true,
    engine,
    mediaType,
    dataBase64: bytes.toString("base64"),
    bytes: bytes.byteLength,
  };
}

export async function shutdown() {
  if (browserPromise) {
    const activeBrowser = await browserPromise;
    await activeBrowser.close();
    browserPromise = undefined;
  }
  if (mathJaxReady) {
    MathJax.done?.();
    mathJaxReady = undefined;
  }
}

function responseForError(request, error) {
  return {
    id: request?.id ?? null,
    ok: false,
    error: error instanceof Error ? error.message : String(error),
  };
}

async function readStandardInput() {
  let input = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => {
    input += chunk;
  });
  await once(process.stdin, "end");
  return input;
}

function optionalInteger(value) {
  if (!value) return undefined;
  const parsed = Number.parseInt(value, 10);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : undefined;
}

async function runStdio() {
  const source = await readStandardInput();
  const request = {
    kind: process.env.PTYMARK_BLOCK_KIND,
    source,
    engine: process.env.PTYMARK_ENGINE_VARIANT,
    width: optionalInteger(process.env.PTYMARK_TERMINAL_WIDTH),
    height: optionalInteger(process.env.PTYMARK_TERMINAL_HEIGHT),
    theme: process.env.PTYMARK_THEME || "default",
  };
  try {
    const response = await renderRequest(request);
    process.stdout.write(Buffer.from(response.dataBase64, "base64"));
  } catch (error) {
    process.stderr.write(
      `${error instanceof Error ? error.message : String(error)}\n`,
    );
    process.exitCode = 1;
  } finally {
    await shutdown();
  }
}

async function runOnce() {
  const input = await readStandardInput();
  const request = JSON.parse(input);
  try {
    process.stdout.write(`${JSON.stringify(await renderRequest(request))}\n`);
  } catch (error) {
    process.stdout.write(`${JSON.stringify(responseForError(request, error))}\n`);
    process.exitCode = 1;
  } finally {
    await shutdown();
  }
}

async function runWorker() {
  const lines = readline.createInterface({
    input: process.stdin,
    crlfDelay: Infinity,
  });
  for await (const line of lines) {
    if (!line.trim()) continue;
    let request;
    try {
      request = JSON.parse(line);
      process.stdout.write(`${JSON.stringify(await renderRequest(request))}\n`);
    } catch (error) {
      process.stdout.write(`${JSON.stringify(responseForError(request, error))}\n`);
    }
  }
  await shutdown();
}

const invokedDirectly =
  process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
if (invokedDirectly) {
  const shutdownOnSignal = async () => {
    await shutdown();
    process.exit(0);
  };
  process.once("SIGINT", shutdownOnSignal);
  process.once("SIGTERM", shutdownOnSignal);

  if (process.argv.includes("--stdio-v1")) {
    await runStdio();
  } else if (process.argv.includes("--once")) {
    await runOnce();
  } else {
    await runWorker();
  }
}
