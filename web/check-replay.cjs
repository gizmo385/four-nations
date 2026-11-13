// The web replayer (#999), checked in headless Chromium: the built page opens with nothing to
// replay, opens a run log through its "Open log…" picker, steps it with the watcher's keys, and
// draws the map, with no console errors on the way. Not part of `cargo xtask`; nothing in CI runs
// it (it needs Playwright and its Chromium):
//
//   cargo xtask web && cargo xtask web-gui --fast    (or `cargo xtask web --serve --fast`)
//   node web/check-replay.cjs LOG [URL]
//
// LOG is a run.jsonl made on this build (a log replays only against the data it was made with).
// With no URL it serves target/replayer/ itself on a free local port and checks its root; with one
// (say the deployed https://gizmo385.github.io/four-nations/) it checks that page. State comes
// from what fn_gui publishes on the canvas (`data-fn-mode`, `-turn`, `-total`, `-status`;
// crates/gui/src/webpage.rs). Exits 1 on the first failed check.

const fs = require('fs');
const http = require('http');
const path = require('path');
const { chromium } = require('playwright');

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.wasm': 'application/wasm',
  '.json': 'application/json',
};

// A static server of `root`, on a free port; resolves to { url, close }.
function serve(root) {
  const server = http.createServer((req, res) => {
    let rel = decodeURIComponent(new URL(req.url, 'http://x').pathname);
    if (rel.endsWith('/')) rel += 'index.html';
    const file = path.join(root, rel);
    if (!file.startsWith(root) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      res.writeHead(404).end('not found');
      return;
    }
    res.writeHead(200, { 'Content-Type': TYPES[path.extname(file)] || 'application/octet-stream' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((resolve) =>
    server.listen(0, '127.0.0.1', () =>
      resolve({
        url: `http://127.0.0.1:${server.address().port}/`,
        close: () => server.close(),
      }),
    ),
  );
}

let checks = 0;
function ok(cond, what) {
  checks += 1;
  if (!cond) {
    console.error(`FAIL: ${what}`);
    process.exitCode = 1;
    throw new Error(what);
  }
  console.log(`ok   ${what}`);
}

const attr = (page, name) => page.getAttribute('#the_canvas', `data-fn-${name}`);

// Waits for `test(arg)` to hold in the page, or fails saying what the canvas holds instead.
async function until(page, what, test, arg, timeout = 120000) {
  await page.waitForFunction(test, arg, { timeout }).catch(async () => {
    const seen = await page.evaluate(() => {
      const c = document.getElementById('the_canvas');
      return c ? [...c.attributes].map((a) => `${a.name}=${a.value}`).join(' ') : 'no canvas';
    });
    throw new Error(`${what} never happened (canvas: ${seen})`);
  });
}

const canvasAttr = ([name, want]) =>
  document.getElementById('the_canvas')?.getAttribute(`data-fn-${name}`) === want;

// How many distinct colours (each channel in steps of 8) the canvas shows right now.
async function colours(page) {
  const png = (await page.locator('#the_canvas').screenshot()).toString('base64');
  return page.evaluate(async (b64) => {
    const blob = await (await fetch(`data:image/png;base64,${b64}`)).blob();
    const bitmap = await createImageBitmap(blob);
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const ctx = canvas.getContext('2d');
    ctx.drawImage(bitmap, 0, 0);
    const { data } = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
    const seen = new Set();
    for (let i = 0; i < data.length; i += 4) {
      seen.add(((data[i] >> 3) << 10) | ((data[i + 1] >> 3) << 5) | (data[i + 2] >> 3));
    }
    return seen.size;
  }, png);
}

(async () => {
  const log = process.argv[2];
  if (!log || !fs.existsSync(log)) {
    console.error('usage: node web/check-replay.cjs LOG [URL]   (LOG: a run.jsonl made on this build)');
    process.exit(2);
  }
  let site = null;
  let base = process.argv[3];
  if (!base) {
    site = await serve(path.resolve(__dirname, '../target/replayer'));
    base = site.url;
  }
  const browser = await chromium.launch();
  const page = await (await browser.newContext({ viewport: { width: 1600, height: 1000 } })).newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(`page error: ${e.message}`));
  // A failed load is logged too; the response check below says which, and allows one.
  page.on('console', (m) => m.type() === 'error' && !m.text().startsWith('Failed to load resource')
    && problems.push(`console error: ${m.text()}`));
  page.on('requestfailed', (r) => problems.push(`request failed: ${r.url()}`));
  // runs/index.json is optional: the published site has none.
  page.on('response', (r) => r.status() >= 400 && !r.url().endsWith('runs/index.json')
    && problems.push(`${r.status()}: ${r.url()}`));
  try {
    // 1. Nothing open yet: the page says how to open a log.
    await page.goto(base);
    await until(page, 'the empty replay', canvasAttr, ['mode', 'replay-empty']);
    ok(true, 'the page opens with nothing to replay');

    // 2. Open the log through the picker: the replay starts.
    await page.setInputFiles('#file', log);
    await until(page, 'the replay', canvasAttr, ['mode', 'replay']);
    await until(page, 'the turn count', () => Number(document.getElementById('the_canvas')?.getAttribute('data-fn-total')) > 0, null, 10000);
    const total = Number(await attr(page, 'total'));
    ok(total > 0, `the log opens (${total} turns)`);

    // 3. Pause, go back to the start, then two turns on: the watcher's keys.
    await page.focus('#the_canvas');
    await page.keyboard.press(' ');
    await page.keyboard.press('{');
    await page.waitForTimeout(500);
    await page.keyboard.press(']');
    await page.waitForTimeout(200);
    await page.keyboard.press(']');
    const want = String(Math.min(2, total));
    await until(page, `turn ${want}`, canvasAttr, ['turn', want], 30000);
    ok(true, `Space, { and ]] step the replay to turn ${want}`);

    // 4. The map is on screen.
    await page.waitForTimeout(500);
    const shown = await colours(page);
    ok(shown >= 16, `the replay draws (${shown} colours)`);

    ok(problems.length === 0, `no console errors${problems.length ? `: ${problems.join('; ')}` : ''}`);
    console.log(`${checks} checks passed`);
  } catch (e) {
    if (problems.length) console.error(`console problems: ${problems.join('; ')}`);
    console.error(e.message);
    process.exitCode = 1;
  } finally {
    await browser.close();
    if (site) site.close();
  }
})();
