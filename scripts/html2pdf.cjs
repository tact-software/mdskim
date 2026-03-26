// HTML to PDF converter using puppeteer-core + system Chrome/Chromium
// Usage: node html2pdf.cjs <input.html> <output.pdf>
const puppeteer = require('puppeteer-core');
const fs = require('fs');

const args = process.argv.slice(2);
const flags = args.filter(a => a.startsWith('--'));
const positional = args.filter(a => !a.startsWith('--'));

if (positional.length < 2) {
  console.error('Usage: node html2pdf.cjs <input.html> <output.pdf> [--no-sandbox]');
  process.exit(1);
}

const [inputHtml, outputPdf] = positional;
const noSandbox = flags.includes('--no-sandbox');

function findChrome() {
  // Environment variable override (highest priority)
  for (const envVar of ['CHROME_PATH', 'PUPPETEER_EXECUTABLE_PATH']) {
    const envPath = process.env[envVar];
    if (envPath && fs.existsSync(envPath)) return envPath;
  }

  const candidates = [
    // macOS
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Chromium.app/Contents/MacOS/Chromium',
    '/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary',
    '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
    // Linux
    '/usr/bin/google-chrome',
    '/usr/bin/google-chrome-stable',
    '/usr/bin/chromium',
    '/usr/bin/chromium-browser',
    '/snap/bin/chromium',
    // Windows
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    'C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe',
    process.env.LOCALAPPDATA + '\\Google\\Chrome\\Application\\chrome.exe',
  ];

  for (const p of candidates) {
    if (p && fs.existsSync(p)) return p;
  }
  return null;
}

(async () => {
  const chromePath = findChrome();
  if (!chromePath) {
    console.error(
      'Error: Chrome/Chromium not found.\n\n' +
      'PDF export requires Google Chrome or Chromium.\n' +
      'Install one of the following:\n' +
      '  - Google Chrome: https://www.google.com/chrome/\n' +
      '  - Chromium:      brew install --cask chromium  (macOS)\n' +
      '                    apt install chromium-browser  (Ubuntu/Debian)\n\n' +
      'Or set CHROME_PATH environment variable to your browser executable.'
    );
    process.exit(1);
  }

  const chromiumArgs = [];
  if (noSandbox) {
    chromiumArgs.push('--no-sandbox', '--disable-setuid-sandbox');
  }

  let browser;
  try {
    browser = await puppeteer.launch({
      executablePath: chromePath,
      headless: true,
      args: chromiumArgs,
    });
  } catch (e) {
    if (e.message && e.message.includes('--no-sandbox')) {
      console.error(
        'Error: Chrome failed to launch (sandbox error).\n\n' +
        'If running as root (e.g. in Docker), add --no-sandbox:\n' +
        '  mdskim doc.md --export-pdf out.pdf --no-sandbox'
      );
    } else {
      console.error('Error: Chrome failed to launch.\n' + e.message);
    }
    process.exit(1);
  }
  const page = await browser.newPage();
  await page.goto(`file://${inputHtml}`, {
    waitUntil: 'networkidle0',
    timeout: 30000,
  });
  await page.pdf({
    path: outputPdf,
    format: 'A4',
    margin: { top: '15mm', bottom: '15mm', left: '15mm', right: '15mm' },
    printBackground: true,
  });
  await browser.close();
})();
