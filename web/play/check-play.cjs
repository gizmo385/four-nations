// Browser play (#866), checked in headless Chromium: the built page opens at the title screen,
// starts a small map from it, and the map draws, with no console errors on the way. Not part of
// `cargo xtask`; nothing in CI runs it (it needs Playwright and its Chromium, which a developer
// machine has and the runner doesn't):
//
//   cargo xtask web-gui --fast               (or without --fast for the deployed build)
//   node web/play/check-play.cjs [URL]
//
// With no URL it serves target/replayer/ itself on a free local port and checks /play/ there;
// with one (say the deployed https://gizmo385.github.io/four-nations/play/) it checks that page.
// It never installs anything: `require('playwright')` and `chromium.launch()` as found.
//
// The game is pixels, so state comes from what fn_gui publishes on the canvas (`data-fn-mode`,
// `-page` on the title screen, `-tick`, `-seed`; crates/gui/src/page.rs), and "the map draws" is read from a screenshot:
// the canvas holds many distinct colours (terrain, units, panel), where a blank or failed one
// holds one or two. Exits 1 on the first failed check.

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
        url: `http://127.0.0.1:${server.address().port}/play/`,
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

// Waits for the title screen's step (`menu`, `nation`, `seed`, `load`): the native title (#880)
// is egui rows, so the step is read from the canvas's attribute, not from the pixels.
async function titlePage(page, want, timeout = 30000) {
  await page
    .waitForFunction(
      (p) => document.getElementById('the_canvas')?.getAttribute('data-fn-page') === p,
      want,
      { timeout },
    )
    .catch(async () => {
      throw new Error(`title step never became ${want}; it is ${JSON.stringify(await attr(page, 'page'))}`);
    });
}

async function mode(page, want, timeout = 180000) {
  await page
    .waitForFunction(
      (m) => document.getElementById('the_canvas')?.getAttribute('data-fn-mode') === m,
      want,
      { timeout },
    )
    .catch(async () => {
      throw new Error(`mode never became ${want}; it is ${JSON.stringify(await attr(page, 'mode'))}`);
    });
}

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
  let site = null;
  let base = process.argv[2];
  if (!base) {
    site = await serve(path.resolve(__dirname, '../../target/replayer'));
    base = site.url;
  }
  const browser = await chromium.launch();
  const page = await (await browser.newContext({ viewport: { width: 1280, height: 800 } })).newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(`page error: ${e.message}`));
  page.on('console', (m) => m.type() === 'error' && problems.push(`console error: ${m.text()}`));
  page.on('requestfailed', (r) => problems.push(`request failed: ${r.url()}`));
  page.on('response', (r) => r.status() >= 400 && problems.push(`${r.status()}: ${r.url()}`));
  try {
    // 1. No query: the title screen, drawn.
    await page.goto(base);
    await mode(page, 'title');
    await titlePage(page, 'menu');
    await page.focus('#the_canvas');
    await page.waitForTimeout(500);
    const title = await colours(page);
    ok(title >= 4, `the title screen draws (${title} colours)`);

    // 2. New game, the first nation, seed 7, the small map (the default size): embark.
    await page.keyboard.press('Enter'); // New game
    await titlePage(page, 'nation');
    await page.keyboard.press('Enter'); // the first nation
    await titlePage(page, 'seed');
    await page.keyboard.type('7');
    await page.keyboard.press('Enter'); // the seed step's default rows: Small, Exact survey
    await mode(page, 'embark');
    ok(true, 'New game opens embark');

    // 3. Settle at the cursor: the game, on seed 7, ticking.
    await page.keyboard.press('e');
    await page.keyboard.press('y');
    await mode(page, 'game');
    ok((await attr(page, 'seed')) === '7', 'settling starts the seed 7 game');
    const t0 = Number(await attr(page, 'tick'));
    await page.waitForFunction(
      (t) => Number(document.getElementById('the_canvas').getAttribute('data-fn-tick')) > t,
      t0,
      { timeout: 30000 },
    );
    ok(true, 'the game runs (its tick advances)');

    // 4. The map is on screen.
    await page.waitForTimeout(500);
    const game = await colours(page);
    ok(game >= 16, `the map draws (${game} colours; the title screen had ${title})`);

    // 5. The game menu opens (its rows, Quit among them, are a fn_tui test: pixels cannot say).
    await page.keyboard.press('Escape');
    await page.waitForTimeout(300);
    const menu = await colours(page);
    ok(menu !== game, `Esc opens the game menu (${menu} colours, was ${game})`);
    await page.keyboard.press('Escape');

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
