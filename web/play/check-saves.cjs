// Browser saves for fn_gui (#860), checked in headless Chromium. Not part of `cargo xtask`:
//
//   cargo xtask web-gui --fast --serve        (one terminal; or any server of target/replayer/)
//   node web/play/check-saves.cjs [http://127.0.0.1:8595/play/]
//
// Needs Playwright (`require('playwright')`) and its Chromium, both preinstalled where this is
// run; it never installs anything. The game is pixels, so it reads the state fn_gui publishes
// on the canvas (`data-fn-mode`, `-tick`, `-seed`, `-status`; crates/gui/src/page.rs).
//
// What it checks: F5 saves into localStorage and says so; a fresh page load of a different
// game (so the state really came from the save) has F9 bring the saved game back; the saves
// screen (F6) saves a named slot with N and loads it from the list after another reload; F5 and
// F6 don't reach the browser; and a full localStorage gives a visible error and keeps the old
// quicksave. Exits 1 on the first failed check.

const { chromium } = require('playwright');

const BASE = process.argv[2] || 'http://127.0.0.1:8595/play/';
const KEY = (name) => `four-nations/saves/${name}`;
const SEED = 7; // the game that gets saved
const OTHER = 9; // the game a reload starts instead

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

// A key press, then the status line it leaves (the canvas attribute changes on the next frame).
async function press(page, key, wantStatus) {
  await page.keyboard.press(key);
  if (wantStatus) await status(page, wantStatus);
}

// The "Load it, losing this game" answer: the box opens on the safe "Keep playing" row.
async function confirm(page) {
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
}

async function status(page, re) {
  await page.waitForFunction(
    (src) => new RegExp(src).test(document.getElementById('the_canvas').getAttribute('data-fn-status') || ''),
    re.source,
    { timeout: 15000 },
  ).catch(async () => {
    throw new Error(`status never matched ${re}; it is ${JSON.stringify(await attr(page, 'status'))}`);
  });
  return attr(page, 'status');
}

(async () => {
  const browser = await chromium.launch();
  const page = await (await browser.newContext()).newPage();
  page.on('pageerror', (e) => console.error('page error:', e.message));
  try {
    // 1. A game, and F5.
    await start(page, SEED);
    ok((await attr(page, 'seed')) === String(SEED), `started the seed ${SEED} game`);
    // F5 and F6 stay in the game (not the browser's reload and address bar); Ctrl+F5 doesn't.
    const prevented = await page.evaluate(() => {
      const canvas = document.getElementById('the_canvas');
      const fire = (init) => !canvas.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }));
      return { f5: fire({ key: 'F5' }), f6: fire({ key: 'F6' }), ctrlF5: fire({ key: 'F5', ctrlKey: true }) };
    });
    ok(prevented.f5 && prevented.f6 && !prevented.ctrlF5, `F5 and F6 are the game's alone: ${JSON.stringify(prevented)}`);
    // (The game saw those synthetic keys too: F6 opened the saves screen, and F5 saved.)
    await page.keyboard.press('F6'); // closes the saves screen again

    await page.waitForTimeout(1500); // let it run a little, so the save isn't tick 0
    const said = await (async () => {
      await press(page, 'F5');
      return status(page, /^Quicksaved to browser storage as quicksave \(tick \d+\)/);
    })();
    const savedTick = Number(/tick (\d+)/.exec(said)[1]);
    ok(savedTick > 0, `F5 quicksaved: "${said}"`);
    const stored = await page.evaluate((k) => localStorage.getItem(k), KEY('quicksave'));
    ok(stored && stored.startsWith('fn-save-1\n'), 'the quicksave is in localStorage');
    console.log(`     stored value: ${stored.length} characters`);

    // 2. A named slot from the saves screen: F6, N, a name, Enter.
    await press(page, 'F6');
    await press(page, 'n');
    await page.keyboard.type('mysave');
    await press(page, 'Enter');
    const named = await status(page, /^Saved to browser storage as mysave \(tick \d+\)/);
    ok(true, `the saves screen saved a slot: "${named}"`);
    const namedTick = Number(/tick (\d+)/.exec(named)[1]);
    await press(page, 'Escape');

    // 3. A fresh page load: a different game, then F9 brings back the saved one.
    await start(page, OTHER);
    ok((await attr(page, 'seed')) === String(OTHER), `a reload starts the seed ${OTHER} game instead`);
    ok(
      (await page.evaluate((k) => localStorage.getItem(k) !== null, KEY('quicksave'))),
      'the quicksave survived the reload',
    );
    await press(page, 'F9');
    await confirm(page);
    const loaded = await status(page, /^Quickloaded \(tick \d+\)/);
    ok(Number(/tick (\d+)/.exec(loaded)[1]) === savedTick, `F9 loaded the quicksave: "${loaded}"`);
    ok((await attr(page, 'seed')) === String(SEED), `and it is the seed ${SEED} game again`);

    // 4. The saves screen's list, after another reload: the newest slot is the one named.
    await start(page, OTHER);
    await press(page, 'F6');
    await press(page, 'Enter');
    await confirm(page);
    const fromList = await status(page, /^Loaded mysave \(tick \d+\)/);
    ok(Number(/tick (\d+)/.exec(fromList)[1]) === namedTick, `the saves screen loaded the slot: "${fromList}"`);
    ok((await attr(page, 'seed')) === String(SEED), `and it is the seed ${SEED} game again`);

    // 5. A full localStorage: a visible error, the old quicksave untouched.
    const before = await page.evaluate((k) => localStorage.getItem(k), KEY('quicksave'));
    const filled = await page.evaluate(() => {
      let total = 0;
      let n = 0;
      for (let size = 1 << 20; size >= 1; size = Math.floor(size / 4)) {
        for (;;) {
          try {
            localStorage.setItem(`filler/${n}`, 'x'.repeat(size));
            n += 1;
            total += size + `filler/${n - 1}`.length;
          } catch (e) {
            break;
          }
        }
      }
      return total;
    });
    console.log(`     filled ${filled} more characters before the quota`);
    // A new slot needs room the quicksave's own key doesn't have to give back.
    await press(page, 'F6');
    await press(page, 'n');
    await page.keyboard.type('toobig');
    await press(page, 'Enter');
    const full = await status(page, /^Save failed: browser storage is full/);
    ok(true, `a full store says so: "${full.slice(0, 90)}..."`);
    const after = await page.evaluate((k) => localStorage.getItem(k), KEY('quicksave'));
    ok(after === before, 'the quicksave is as it was');
    ok(
      (await page.evaluate((k) => localStorage.getItem(k), KEY('toobig'))) === null,
      'and nothing was half written',
    );
    await page.evaluate(() => {
      for (const k of Object.keys(localStorage)) if (k.startsWith('filler/')) localStorage.removeItem(k);
    });
    await start(page, SEED);
    await press(page, 'F5');
    await status(page, /^Quicksaved to browser storage as quicksave \(tick \d+\)/);
    ok(true, 'with room again, F5 saves again');
    console.log(`\n${checks} checks passed`);
  } catch (e) {
    process.exitCode = 1;
    console.error(e.message);
    await page.screenshot({ path: 'check-saves-failure.png' }).catch(() => {});
  } finally {
    await browser.close();
  }
})();
