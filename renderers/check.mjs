import { Buffer } from "node:buffer";
import katex from "katex";
import { renderRequest, shutdown } from "./worker.mjs";

function decode(response) {
  if (!response.ok) throw new Error(response.error || "renderer failed");
  const output = Buffer.from(response.dataBase64, "base64").toString("utf8");
  if (!output.includes("<svg")) {
    throw new Error(`renderer ${response.engine} did not produce SVG`);
  }
  return output;
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
  console.log("ptymark renderer engines: ok");
} finally {
  await shutdown();
}
