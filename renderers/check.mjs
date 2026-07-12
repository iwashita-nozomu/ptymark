import { Buffer } from "node:buffer";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

import katex from "katex";
import { renderRequest, shutdown } from "./worker.mjs";

const workerPath = fileURLToPath(new URL("./worker.mjs", import.meta.url));

function decode(response) {
  if (!response.ok) throw new Error(response.error || "renderer failed");
  const output = Buffer.from(response.dataBase64, "base64").toString("utf8");
  if (!output.includes("<svg")) {
    throw new Error(`renderer ${response.engine} did not produce SVG`);
  }
  return output;
}

async function renderStdio(kind, source, variant) {
  const child = spawn(process.execPath, [workerPath, "--stdio-v1"], {
    stdio: ["pipe", "pipe", "pipe"],
    env: {
      ...process.env,
      PTYMARK_RENDERER_PROTOCOL: "stdio-v1",
      PTYMARK_RENDERER_ID: `check/${variant}`,
      PTYMARK_BLOCK_KIND: kind,
      PTYMARK_ENGINE_VARIANT: variant,
      PTYMARK_SOURCE_BYTES: String(Buffer.byteLength(source)),
      PTYMARK_TERMINAL_WIDTH: "800",
    },
  });
  const stdout = [];
  const stderr = [];
  child.stdout.on("data", (chunk) => stdout.push(chunk));
  child.stderr.on("data", (chunk) => stderr.push(chunk));
  child.stdin.end(source);
  const code = await new Promise((resolve) => child.once("exit", resolve));
  if (code !== 0) {
    throw new Error(
      `stdio renderer ${variant} exited ${code}: ${Buffer.concat(stderr).toString("utf8")}`,
    );
  }
  return Buffer.concat(stdout).toString("utf8");
}

try {
  decode(
    await renderRequest({
      id: 1,
      kind: "math",
      source: String.raw`\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}`,
    }),
  );
  decode(
    await renderRequest({
      id: 2,
      kind: "mermaid",
      source: "flowchart LR\n  PTY --> Detector --> Renderer --> Display",
      width: 800,
      height: 600,
      theme: "default",
    }),
  );

  const mathml = katex.renderToString(String.raw`E = mc^2`, {
    displayMode: true,
    output: "mathml",
    throwOnError: true,
  });
  if (!mathml.includes("<math")) {
    throw new Error("KaTeX comparator did not produce MathML");
  }

  const stdioMath = await renderStdio("math", String.raw`E = mc^2`, "katex");
  if (!stdioMath.includes("<math")) {
    throw new Error("KaTeX stdio-v1 adapter did not produce MathML");
  }
  const stdioMermaid = await renderStdio(
    "mermaid",
    "flowchart LR\n  Input --> Gate --> Output",
    "mermaid",
  );
  if (!stdioMermaid.includes("<svg")) {
    throw new Error("Mermaid stdio-v1 adapter did not produce SVG");
  }

  console.log("ptymark renderer engines: ok");
} finally {
  await shutdown();
}
