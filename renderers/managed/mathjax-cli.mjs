import MathJax from '@mathjax/src/source';

const formula = process.argv[2] ?? '';
if (formula.length === 0) {
  console.error('ptymark managed MathJax: missing TeX expression');
  process.exit(2);
}

try {
  await MathJax.init({
    loader: { load: ['input/tex', 'output/svg'] },
    svg: { fontCache: 'local' },
  });
  const node = MathJax.tex2svg(formula, { display: true });
  const markup = MathJax.startup.adaptor.serializeXML(node);
  const start = markup.indexOf('<svg');
  const close = markup.lastIndexOf('</svg>');
  if (start < 0 || close < start) {
    throw new Error('MathJax did not produce an SVG element');
  }
  const svg = markup.slice(start, close + '</svg>'.length);
  process.stdout.write(svg);
} catch (error) {
  console.error(`ptymark managed MathJax: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
} finally {
  MathJax.done?.();
}
