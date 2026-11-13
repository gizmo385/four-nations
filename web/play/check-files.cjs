// Saves as files (#976), checked in headless Chromium: the saves screen's Export to file
// downloads the highlighted slot as NAME.fnsave (the bytes localStorage holds); with
// localStorage cleared, Import file adds it back as a slot, which loads; a file that isn't a
// save is refused with a status line and adds nothing. Not part of `cargo xtask`:
//
//   cargo xtask web-gui --fast --serve        (one terminal; or any server of target/replayer/)
//   node web/play/check-files.cjs [http://127.0.0.1:8595/play/]
//
// The buttons are pixels, so the script taps them where the page says they are
// (`data-fn-file-buttons`, crates/gui/src/webpage.rs). Exits 1 on the first failed check.

const fs = require('fs');
const os = require('os');
const path = require('path');
const { chromium } = require('playwright');

const BASE = process.argv[2] || 'http://127.0.0.1:8595/play/';
const KEY = (name) => `four-nations/saves/${name}`;

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

async function start(page, seed) {
  await page.goto(`${BASE}?seed=${seed}&nation=2&size=0`);
  await page.waitForFunction(
    () => document.getElementById('the_canvas')?.getAttribute('data-fn-mode') === 'game',
    null,
    { timeout: 180000 },
  );
  await page.focus('#the_canvas');
}

async function status(page, re) {
  await page
    .waitForFunction(
      (src) => new RegExp(src).test(document.getElementById('the_canvas').getAttribute('data-fn-status') || ''),
      re.source,
      { timeout: 15000 },
    )
    .catch(async () => {
      throw new Error(`status never matched ${re}; it is ${JSON.stringify(await attr(page, 'status'))}`);
    });
  return attr(page, 'status');
}

// Where the saves screen's `which` button (export or import) is, once the screen shows it.
async function button(page, which) {
  await page.waitForFunction(
    () => (document.getElementById('the_canvas').getAttribute('data-fn-file-buttons') || '') !== '',
    null,
    { timeout: 15000 },
  );
  const words = (await attr(page, 'file-buttons')).split(' ');
  const at = words.indexOf(which);
  return { x: Number(words[at + 1]), y: Number(words[at + 2]) };
}

async function tap(page, which) {
  const { x, y } = await button(page, which);
  await page.mouse.move(x, y);
  await page.waitForTimeout(100);
  await page.mouse.down();
  await page.waitForTimeout(150); // a frame sees the press and arms the action
  await page.mouse.up();
}

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fn-files-'));
  const browser = await chromium.launch();
  const page = await (await browser.newContext({ acceptDownloads: true })).newPage();
  page.on('pageerror', (e) => console.error('page error:', e.message));
  try {
    // 1. A game, quicksaved, and the saves screen.
    await start(page, 7);
    await page.waitForTimeout(1000);
    await page.keyboard.press('F5');
    const said = await status(page, /^Quicksaved to browser storage as quicksave \(tick \d+\)/);
    const savedTick = Number(/tick (\d+)/.exec(said)[1]);
    await page.keyboard.press('F6');

    // 2. Export to file: a download named quicksave.fnsave, the stored bytes.
    const [download] = await Promise.all([page.waitForEvent('download'), tap(page, 'export')]);
    ok(download.suggestedFilename() === 'quicksave.fnsave', `the file is ${download.suggestedFilename()}`);
    const file = path.join(dir, 'quicksave.fnsave');
    await download.saveAs(file);
    const stored = await page.evaluate((k) => localStorage.getItem(k), KEY('quicksave'));
    ok(fs.readFileSync(file, 'utf8') === stored, 'its bytes are what localStorage holds');
    await status(page, /^Exported quicksave\.fnsave/);

    // 3. Storage cleared, a reload, Import file: the slot is back, and loads.
    await page.evaluate(() => localStorage.clear());
    await start(page, 9);
    await page.keyboard.press('F6');
    const [chooser] = await Promise.all([page.waitForEvent('filechooser'), tap(page, 'import')]);
    await chooser.setFiles(file);
    await status(page, /^Imported quicksave\.fnsave as quicksave/);
    ok(
      (await page.evaluate((k) => localStorage.getItem(k), KEY('quicksave'))) === stored,
      'the imported slot is in localStorage, byte for byte',
    );
    await page.keyboard.press('Enter');
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Enter');
    const loaded = await status(page, /^Loaded quicksave \(tick \d+\)/);
    ok(Number(/tick (\d+)/.exec(loaded)[1]) === savedTick, `it loads: "${loaded}"`);
    ok((await attr(page, 'seed')) === '7', 'and it is the seed 7 game again');

    // 4. A file that isn't a save: refused, nothing added.
    const junk = path.join(dir, 'notes.fnsave');
    fs.writeFileSync(junk, 'hello');
    const before = await page.evaluate(() => Object.keys(localStorage).length);
    await page.keyboard.press('F6');
    const [again] = await Promise.all([page.waitForEvent('filechooser'), tap(page, 'import')]);
    await again.setFiles(junk);
    const refused = await status(page, /^notes\.fnsave isn't a Four Nations save/);
    ok(true, `a file that isn't a save is refused: "${refused}"`);
    ok((await page.evaluate(() => Object.keys(localStorage).length)) === before, 'and no slot was added');

    console.log(`\n${checks} checks passed`);
  } catch (e) {
    process.exitCode = 1;
    console.error(e.message);
    await page.screenshot({ path: path.join(dir, 'failure.png') }).catch(() => {});
    console.error(`screenshot: ${path.join(dir, 'failure.png')}`);
  } finally {
    await browser.close();
  }
})();
