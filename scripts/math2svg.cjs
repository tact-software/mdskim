#!/usr/bin/env node
// mdskim math renderer — converts LaTeX to SVG via MathJax v3
// Usage: node math2svg.cjs <input.tex> <output.svg> [--display]
// Requires: npm install -g mathjax-full

const fs = require('fs');

const args = process.argv.slice(2);
if (args.length < 2) {
  console.error('Usage: math2svg.cjs <input.tex> <output.svg> [--display]');
  process.exit(1);
}

const inputPath = args[0];
const outputPath = args[1];
const displayMode = args.includes('--display');
const tex = fs.readFileSync(inputPath, 'utf8').trim();

const { mathjax } = require('mathjax-full/js/mathjax.js');
const { TeX } = require('mathjax-full/js/input/tex.js');
const { SVG } = require('mathjax-full/js/output/svg.js');
const { liteAdaptor } = require('mathjax-full/js/adaptors/liteAdaptor.js');
const { RegisterHTMLHandler } = require('mathjax-full/js/handlers/html.js');
const { AllPackages } = require('mathjax-full/js/input/tex/AllPackages.js');

const adaptor = liteAdaptor();
RegisterHTMLHandler(adaptor);

const texInput = new TeX({ packages: AllPackages });
const svgOutput = new SVG({ fontCache: 'none' });
const doc = mathjax.document('', { InputJax: texInput, OutputJax: svgOutput });

const node = doc.convert(tex, { display: displayMode });
const svgStr = adaptor.outerHTML(node);

// Extract inner <svg> from <mjx-container>
const svgMatch = svgStr.match(/<svg[\s\S]*<\/svg>/);
fs.writeFileSync(outputPath, svgMatch ? svgMatch[0] : svgStr);
