// The installable app (#974), checked in headless Chromium: the page links its manifest, the
// service worker takes the page over, and with the network off a reload still starts the game.
// Like check-play.cjs, not part of `cargo xtask` and nothing in CI runs it:
//
//   cargo xtask web-gui --fast
//   node web/play/check-offline.cjs [URL]
//
// With no URL it serves target/replayer/ on a free local port (localhost counts as secure, so
// a service worker may register) and checks /play/ there. Exits 1 on the first failed check.

const fs = require('fs');
const http = require('http');
const path = require('path');
const { chromium } = require('playwright');

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.webmanifest': 'application/manifest+json',
  '.png': 'image/png',
};

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

// Waits until the game on seed 7 is ticking.
async function ticking(page) {
  await page.waitForFunction(
    () => {
      const c = document.getElementById('the_canvas');
      return c?.getAttribute('data-fn-mode') === 'game' && Number(c.getAttribute('data-fn-tick')) > 0;
    },
    null,
    { timeout: 180000 },
  );
}

(async () => {
  let site = null;
  let base = process.argv[2];
  if (!base) {
    site = await serve(path.resolve(__dirname, '../../target/replayer'));
    base = site.url;
  }
  const game = `${base}?seed=7&nation=0&size=0`;
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 844, height: 390 } });
  const page = await context.newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(`page error: ${e.message}`));
  try {
    await page.goto(game);
    await ticking(page);
    ok(true, 'the game starts online');

    const manifest = await page.evaluate(async () => {
      const link = document.querySelector('link[rel=manifest]');
      return link ? (await fetch(link.href)).json() : null;
    });
    ok(manifest?.display === 'standalone' && manifest?.orientation === 'landscape',
      'the manifest is linked: standalone, landscape');

    await page.evaluate(() => navigator.serviceWorker.ready);
    await page.waitForFunction(() => navigator.serviceWorker.controller !== null, null, {
      timeout: 30000,
    }).catch(() => page.reload().then(() => ticking(page)));
    ok(await page.evaluate(() => navigator.serviceWorker.controller !== null),
      'the service worker controls the page');
    const cached = await page.evaluate(async () => {
      const names = await caches.keys();
      const cache = await caches.open(names.find((n) => n.startsWith('fn-play-')));
      return (await cache.keys()).map((r) => new URL(r.url).pathname + new URL(r.url).search);
    });
    ok(cached.some((u) => /fn_gui_bg\.wasm\?v=[0-9a-f]{16}$/.test(u)),
      `the hashed wasm is cached (${cached.length} files)`);

    await context.setOffline(true);
    await page.reload();
    await ticking(page);
    ok(true, 'offline, a reload still starts the game');

    ok(problems.length === 0, `no page errors${problems.length ? `: ${problems.join('; ')}` : ''}`);
    console.log(`${checks} checks passed`);
  } catch (e) {
    if (problems.length) console.error(`page problems: ${problems.join('; ')}`);
    console.error(e.message);
    process.exitCode = 1;
  } finally {
    await browser.close();
    if (site) site.close();
  }
})();
