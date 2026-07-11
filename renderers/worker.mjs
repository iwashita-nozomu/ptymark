import { Buffer } from "node:buffer";
import { once } from "node:events";
import process from "node:process";
import readline from "node:readline";
import { pathToFileURL } from "node:url";

import MathJax from "@mathjax/src/source";
import { renderMermaid } from "@mermaid-js/mermaid-cli";
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

async function renderMath(source) {
  const engine = await mathJax();
  const node = engine.tex2svg(source, { display: true });
  return engine.startup.adaptor.serializeXML(node);
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

  let bytes;
  let mediaType;
  switch (request.kind) {
    case "math":
      bytes = Buffer.from(await renderMath(request.source), "utf8");
      mediaType = "image/svg+xml";
      break;
    case "mermaid":
      bytes = await renderDiagram(request);
      mediaType = "image/svg+xml";
      break;
    default:
      throw new Error(`unsupported renderer kind: ${request.kind}`);
  }

  return {
    id: request.id ?? null,
    ok: true,
    engine:
      request.kind === "math"
        ? "mathjax/4.1.3"
        : "mermaid-cli/11.16.0-persistent",
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

async function runOnce() {
  let input = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => {
    input += chunk;
  });
  await once(process.stdin, "end");
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
  const onceMode = process.argv.includes("--once");
  const shutdownOnSignal = async () => {
    await shutdown();
    process.exit(0);
  };
  process.once("SIGINT", shutdownOnSignal);
  process.once("SIGTERM", shutdownOnSignal);
  await (onceMode ? runOnce() : runWorker());
}
