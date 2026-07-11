import { Buffer } from "node:buffer";
import { spawn } from "node:child_process";
import process from "node:process";
import readline from "node:readline";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";

const workerPath = fileURLToPath(new URL("./worker.mjs", import.meta.url));
const iterations = Number.parseInt(process.env.PTYMARK_BENCH_ITERATIONS || "8", 10);
const oneShotIterations = Number.parseInt(
  process.env.PTYMARK_BENCH_ONESHOT_ITERATIONS || "2",
  10,
);

function percentile(sorted, fraction) {
  const index = Math.round((sorted.length - 1) * fraction);
  return sorted[index];
}

function summary(samples, bytes) {
  const sorted = [...samples].sort((a, b) => a - b);
  const mean = sorted.reduce((sum, value) => sum + value, 0) / sorted.length;
  return {
    iterations: sorted.length,
    meanMs: Number(mean.toFixed(3)),
    p50Ms: Number(percentile(sorted, 0.5).toFixed(3)),
    p95Ms: Number(percentile(sorted, 0.95).toFixed(3)),
    maxMs: Number(sorted.at(-1).toFixed(3)),
    artifactBytes: bytes,
  };
}

class WorkerClient {
  constructor() {
    this.child = spawn(process.execPath, [workerPath], {
      stdio: ["pipe", "pipe", "inherit"],
      env: process.env,
    });
    this.lines = readline.createInterface({ input: this.child.stdout });
    this.iterator = this.lines[Symbol.asyncIterator]();
    this.id = 0;
  }

  async render(kind, source) {
    const id = ++this.id;
    const request = {
      id,
      kind,
      source,
      width: 800,
      height: 600,
      theme: "default",
    };
    const started = performance.now();
    this.child.stdin.write(`${JSON.stringify(request)}\n`);
    const { value, done } = await this.iterator.next();
    if (done) throw new Error("renderer worker closed unexpectedly");
    const response = JSON.parse(value);
    if (!response.ok) throw new Error(response.error || "renderer worker failed");
    if (response.id !== id) throw new Error("renderer response ID mismatch");
    return {
      elapsedMs: performance.now() - started,
      bytes: Buffer.from(response.dataBase64, "base64").byteLength,
    };
  }

  async close() {
    this.child.stdin.end();
    await new Promise((resolve, reject) => {
      this.child.once("exit", (code) =>
        code === 0 ? resolve() : reject(new Error(`worker exited ${code}`)),
      );
    });
  }
}

async function benchmarkPersistent(kind, source) {
  const client = new WorkerClient();
  try {
    await client.render(kind, source);
    const samples = [];
    let bytes = 0;
    for (let index = 0; index < iterations; index += 1) {
      const result = await client.render(kind, source);
      samples.push(result.elapsedMs);
      bytes = result.bytes;
    }
    return summary(samples, bytes);
  } finally {
    await client.close();
  }
}

async function benchmarkOneShot(kind, source) {
  const samples = [];
  let bytes = 0;
  for (let index = 0; index < oneShotIterations; index += 1) {
    const request = JSON.stringify({
      id: index + 1,
      kind,
      source,
      width: 800,
      height: 600,
      theme: "default",
    });
    const started = performance.now();
    const child = spawn(process.execPath, [workerPath, "--once"], {
      stdio: ["pipe", "pipe", "inherit"],
      env: process.env,
    });
    let output = "";
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      output += chunk;
    });
    child.stdin.end(request);
    await new Promise((resolve, reject) => {
      child.once("exit", (code) =>
        code === 0 ? resolve() : reject(new Error(`one-shot exited ${code}`)),
      );
    });
    const response = JSON.parse(output.trim());
    if (!response.ok) throw new Error(response.error || "one-shot failed");
    samples.push(performance.now() - started);
    bytes = Buffer.from(response.dataBase64, "base64").byteLength;
  }
  return summary(samples, bytes);
}

const sources = {
  math: String.raw`\int_0^\infty e^{-x^2} \, dx = \frac{\sqrt{\pi}}{2}`,
  mermaid:
    "flowchart LR\n  Input --> Gate --> Detector --> Coordinator --> Presenter --> Terminal",
};

const result = {
  schema: "ptymark.renderer-benchmark.v1",
  node: process.version,
  iterations,
  oneShotIterations,
  engines: {},
};
for (const [kind, source] of Object.entries(sources)) {
  result.engines[kind] = {
    persistent: await benchmarkPersistent(kind, source),
    oneShot: await benchmarkOneShot(kind, source),
  };
}
console.log(JSON.stringify(result, null, 2));
