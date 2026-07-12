import MathJax from "@mathjax/src/source";

const ready = MathJax.init({
  loader: { load: ["input/tex", "output/svg"] },
  svg: { fontCache: "local" },
});
await ready;

const node = MathJax.tex2svg(String.raw`\int_0^\infty e^{-x^2}\,dx`, {
  display: true,
});
const svg = MathJax.startup.adaptor.serializeXML(node);
if (!svg.includes("<svg")) {
  throw new Error("MathJax did not produce SVG");
}
MathJax.done?.();
console.log("ptymark MathJax smoke: ok");
