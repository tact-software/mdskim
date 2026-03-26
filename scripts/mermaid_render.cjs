#!/usr/bin/env node
// mdskim Mermaid renderer — converts Mermaid diagram source to SVG/PNG
// Usage: node mermaid_render.cjs <input.mmd> <output.svg|png> [--chrome <path>]
// Requires: npm install mermaid puppeteer-core

const fs = require('fs');
const path = require('path');

const args = process.argv.slice(2);
const flags = {};
const positional = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--chrome' && i + 1 < args.length) {
    flags.chrome = args[++i];
  } else if (!args[i].startsWith('--')) {
    positional.push(args[i]);
  }
}

if (positional.length < 2) {
  console.error('Usage: mermaid_render.cjs <input.mmd> <output.svg|png> [--chrome <path>]');
  process.exit(1);
}

const [inputPath, outputPath] = positional;
const source = fs.readFileSync(inputPath, 'utf8').trim();
const format = path.extname(outputPath).slice(1).toLowerCase(); // 'svg' or 'png'

function findChrome() {
  if (flags.chrome && fs.existsSync(flags.chrome)) return flags.chrome;
  for (const v of ['CHROME_PATH', 'PUPPETEER_EXECUTABLE_PATH']) {
    const p = process.env[v];
    if (p && fs.existsSync(p)) return p;
  }
  const candidates = [
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Chromium.app/Contents/MacOS/Chromium',
    '/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary',
    '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
    '/usr/bin/google-chrome',
    '/usr/bin/google-chrome-stable',
    '/usr/bin/chromium',
    '/usr/bin/chromium-browser',
    '/snap/bin/chromium',
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) return c;
  }
  return null;
}

(async () => {
  const puppeteer = require('puppeteer-core');
  const chromePath = findChrome();
  if (!chromePath) {
    console.error(
      'Error: Chrome/Chromium not found.\n' +
      'Mermaid rendering requires a system browser.\n' +
      'Install: brew install --cask google-chrome  (macOS)\n' +
      '         apt install chromium-browser        (Linux)\n' +
      'Or set CHROME_PATH environment variable.'
    );
    process.exit(1);
  }

  let browser;
  try {
    const launchArgs = ['--disable-setuid-sandbox'];
    if (process.env.MDSKIM_NO_SANDBOX === '1') {
      launchArgs.push('--no-sandbox');
    }
    browser = await puppeteer.launch({
      executablePath: chromePath,
      headless: true,
      args: launchArgs,
    });
  } catch (e) {
    console.error('Failed to launch Chrome: ' + e.message);
    process.exit(1);
  }

  try {
    const page = await browser.newPage();
    // Load mermaid from node_modules
    const mermaidPath = require.resolve('mermaid/dist/mermaid.min.js');
    const mermaidJs = fs.readFileSync(mermaidPath, 'utf8');

    await page.setContent(`
      <!DOCTYPE html>
      <html><body>
        <div id="mermaid-container"></div>
        <script>${mermaidJs}</script>
      </body></html>
    `, { waitUntil: 'domcontentloaded' });

    // Render diagram
    const svgResult = await page.evaluate(async (src) => {
      mermaid.initialize({ startOnLoad: false, theme: 'default' });
      try {
        const { svg } = await mermaid.render('mdskim-diagram', src);
        return { ok: true, svg };
      } catch (e) {
        return { ok: false, error: e.message || String(e) };
      }
    }, source);

    if (!svgResult.ok) {
      console.error('Mermaid render error: ' + svgResult.error);
      process.exit(1);
    }

    if (format === 'svg') {
      fs.writeFileSync(outputPath, svgResult.svg);
    } else if (format === 'png') {
      // Set SVG and screenshot
      await page.setContent(`
        <!DOCTYPE html>
        <html><body style="margin:0;padding:0;background:white;">
          ${svgResult.svg}
        </body></html>
      `, { waitUntil: 'networkidle0' });

      const svgEl = await page.$('svg');
      if (svgEl) {
        await svgEl.screenshot({ path: outputPath, type: 'png', omitBackground: false });
      } else {
        await page.screenshot({ path: outputPath, type: 'png', fullPage: true });
      }
    } else {
      console.error('Unsupported format: ' + format);
      process.exit(1);
    }
  } finally {
    await browser.close();
  }
})();
