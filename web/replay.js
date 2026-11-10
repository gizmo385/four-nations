// The web replayer's page (#595): loads fn_web.wasm, feeds it a run log, and draws its frames
// onto a canvas. The wasm side (crates/web/src/abi.rs) does all the replaying; this file only
// moves bytes, keys and time across, and paints cells.
//
// Cell layout (crates/web/src/encode.rs): 12 bytes a cell -- char | style flags << 24, fg, bg --
// with colours as 0x00RRGGBB or DEFAULT.

const CELL_BYTES = 12;
const DEFAULT = 0x0100_0000;
const STYLE = { BOLD: 1 << 24, DIM: 1 << 25, ITALIC: 1 << 26, UNDERLINE: 1 << 27, CROSSED: 1 << 28 };
const CHAR_MASK = 0x00ff_ffff;
const SPECIAL = 0x11_0000;
const SPECIAL_KEYS = {
  Enter: 0, Escape: 1, ArrowLeft: 2, ArrowRight: 3, ArrowUp: 4, ArrowDown: 5, Tab: 6,
  Backspace: 8, Home: 9, End: 10, PageUp: 11, PageDown: 12, Delete: 13,
};
// How long a frame may spend on a seek before drawing, in ms.
const SEEK_BUDGET = 12;
const FONT_FAMILY = '"DejaVu Sans Mono", "Menlo", "Consolas", "Liberation Mono", monospace';

const $ = (id) => document.getElementById(id);
const css = getComputedStyle(document.documentElement);
const DEFAULT_FG = css.getPropertyValue('--fg').trim() || '#d0d0d0';
const DEFAULT_BG = css.getPropertyValue('--bg').trim() || '#121212';

let wasm = null; // the instance's exports
let wasmModule = null; // the compiled module, which the timeline's scan worker instantiates too
let loaded = false;
let lastTime = null;
let prevFrame = null; // the last frame drawn, to repaint only cells that changed
let grid = { cols: 0, rows: 0, cw: 0, ch: 0, font: 0 };
let scrubbing = false;
let crashed = false;

const canvas = $('screen');
const ctx = canvas.getContext('2d', { alpha: false });

// ---------------------------------------------------------------------------------------------
// wasm plumbing
// ---------------------------------------------------------------------------------------------

async function loadWasm() {
  const url = new URL('fn_web.wasm', import.meta.url);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`couldn't load ${url}: ${response.status}`);
  // Streaming needs the server to send application/wasm; fall back to bytes if it doesn't.
  const bytes = response.clone();
  try {
    wasmModule = await WebAssembly.compileStreaming(response);
  } catch (_) {
    wasmModule = await WebAssembly.compile(await bytes.arrayBuffer());
  }
  return (await WebAssembly.instantiate(wasmModule, {})).exports;
}

function text() {
  const bytes = new Uint8Array(wasm.memory.buffer, wasm.fw_text_ptr(), wasm.fw_text_len());
  return new TextDecoder().decode(bytes);
}

// Calls into wasm, turning a trap into a message on screen instead of a dead page.
function call(f) {
  if (crashed) return undefined;
  try {
    return f();
  } catch (e) {
    crashed = true;
    let why = String(e);
    try {
      if (wasm.fw_panic() > 0) why = text();
    } catch (_) { /* the panic message is best effort */ }
    showMessage(`The replayer crashed:\n${why}`, true);
    return undefined;
  }
}

async function openLog(bytes, name) {
  if (!wasm) return;
  bytes = await maybeGunzip(bytes);
  crashed = false;
  const ok = call(() => {
    const ptr = wasm.fw_input(bytes.length);
    new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
    return wasm.fw_open();
  });
  if (ok === undefined) return;
  if (!ok) {
    loaded = false;
    $('controls').hidden = true;
    startScan(null);
    showMessage(`Can't replay ${name}:\n${text()}`, true);
    return;
  }
  const build = text();
  $('build').textContent = `${name}${build ? ` · made on ${build}` : ''}`;
  document.title = `${name} · Four Nations replays`;
  loaded = true;
  prevFrame = null;
  lastTime = null;
  $('controls').hidden = false;
  $('scrub').max = String(wasm.fw_total_turns());
  showMessage('');
  canvas.focus();
  startScan(bytes);
}

async function maybeGunzip(bytes) {
  if (bytes.length < 2 || bytes[0] !== 0x1f || bytes[1] !== 0x8b) return bytes;
  const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

async function openUrl(url) {
  showMessage(`Loading ${url}…`);
  const response = await fetch(url);
  if (!response.ok) {
    showMessage(`Couldn't load ${url}: ${response.status} ${response.statusText}`, true);
    return;
  }
  // A server may already have undone a .gz's encoding; maybeGunzip checks the bytes themselves.
  await openLog(new Uint8Array(await response.arrayBuffer()), url.split('/').pop());
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

function hex(c, fallback) {
  return c === DEFAULT ? fallback : `#${c.toString(16).padStart(6, '0')}`;
}

// Picks the grid: a fixed size, or as many cells as fit the stage at a readable font size.
function layout() {
  const choice = $('size').value;
  const stage = $('stage');
  const dpr = window.devicePixelRatio || 1;
  let cols;
  let rows;
  let font;
  const measure = (px) => {
    ctx.font = `${px}px ${FONT_FAMILY}`;
    return { cw: Math.ceil(ctx.measureText('M').width), ch: Math.ceil(px * 1.25) };
  };
  if (choice === 'fit') {
    font = 14;
    let m = measure(font);
    // Shrink the font until the studio fits (data/tui.toml [studio] min_width x min_height,
    // 160x45): below that the script, game and feed panes collapse into tabs.
    while (font > 8 && (stage.clientWidth / m.cw < 160 || stage.clientHeight / m.ch < 45)) {
      font -= 1;
      m = measure(font);
    }
    cols = Math.max(80, Math.floor(stage.clientWidth / m.cw));
    rows = Math.max(24, Math.floor(stage.clientHeight / m.ch));
  } else {
    [cols, rows] = choice.split('x').map(Number);
    font = 14;
    let m = measure(font);
    while (font > 8 && (cols * m.cw > stage.clientWidth || rows * m.ch > stage.clientHeight)) {
      font -= 1;
      m = measure(font);
    }
  }
  const { cw, ch } = measure(font);
  if (grid.cols === cols && grid.rows === rows && grid.font === font && grid.dpr === dpr) return;
  grid = { cols, rows, cw, ch, font, dpr };
  canvas.width = cols * cw * dpr;
  canvas.height = rows * ch * dpr;
  canvas.style.width = `${cols * cw}px`;
  canvas.style.height = `${rows * ch}px`;
  prevFrame = null;
}

function draw() {
  const ptr = call(() => wasm.fw_render(grid.cols, grid.rows));
  if (ptr === undefined) return;
  const cells = grid.cols * grid.rows;
  const words = new Uint32Array(wasm.memory.buffer, ptr, cells * 3);
  const { cw, ch, font, dpr } = grid;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.textBaseline = 'middle';
  for (let i = 0; i < cells; i++) {
    const w0 = words[i * 3];
    const fg = words[i * 3 + 1];
    const bg = words[i * 3 + 2];
    if (prevFrame && prevFrame[i * 3] === w0 && prevFrame[i * 3 + 1] === fg && prevFrame[i * 3 + 2] === bg) {
      continue;
    }
    const x = (i % grid.cols) * cw;
    const y = Math.floor(i / grid.cols) * ch;
    ctx.fillStyle = hex(bg, DEFAULT_BG);
    ctx.fillRect(x, y, cw, ch);
    const code = w0 & CHAR_MASK;
    if (code > 32) {
      const glyph = String.fromCodePoint(code);
      ctx.font = `${w0 & STYLE.ITALIC ? 'italic ' : ''}${w0 & STYLE.BOLD ? 'bold ' : ''}${font}px ${FONT_FAMILY}`;
      ctx.globalAlpha = w0 & STYLE.DIM ? 0.6 : 1;
      ctx.fillStyle = hex(fg, DEFAULT_FG);
      if (!drawBox(code, x, y, cw, ch)) ctx.fillText(glyph, x, y + ch / 2 + 1);
      if (w0 & STYLE.UNDERLINE) ctx.fillRect(x, y + ch - 2, cw, 1);
      if (w0 & STYLE.CROSSED) ctx.fillRect(x, y + ch / 2, cw, 1);
      ctx.globalAlpha = 1;
    }
  }
  prevFrame = new Uint32Array(words); // a copy: the wasm buffer is reused next frame
}

// Box-drawing lines drawn as rectangles, so borders join up whatever the font's metrics.
// `true` if `code` was one of them.
const BOX = (() => {
  // [left, right, up, down] arms: 1 light, 2 heavy/double.
  const m = new Map();
  const add = (chars, arms) => [...chars].forEach((c) => m.set(c.codePointAt(0), arms));
  add('─', [1, 1, 0, 0]); add('│', [0, 0, 1, 1]);
  add('┌╭', [0, 1, 0, 1]); add('┐╮', [1, 0, 0, 1]); add('└╰', [0, 1, 1, 0]); add('┘╯', [1, 0, 1, 0]);
  add('├', [0, 1, 1, 1]); add('┤', [1, 0, 1, 1]); add('┬', [1, 1, 0, 1]); add('┴', [1, 1, 1, 0]);
  add('┼', [1, 1, 1, 1]);
  add('━', [2, 2, 0, 0]); add('┃', [0, 0, 2, 2]); add('┏', [0, 2, 0, 2]); add('┓', [2, 0, 0, 2]);
  add('┗', [0, 2, 2, 0]); add('┛', [2, 0, 2, 0]);
  add('═', [2, 2, 0, 0]); add('║', [0, 0, 2, 2]); add('╔', [0, 2, 0, 2]); add('╗', [2, 0, 0, 2]);
  add('╚', [0, 2, 2, 0]); add('╝', [2, 0, 2, 0]);
  return m;
})();

function drawBox(code, x, y, cw, ch) {
  const arms = BOX.get(code);
  if (!arms) return false;
  const [l, r, u, d] = arms;
  const cx = Math.floor(x + cw / 2);
  const cy = Math.floor(y + ch / 2);
  const t = (n) => (n === 2 ? 2 : 1);
  if (l) ctx.fillRect(x, cy, cx - x + t(l), t(l));
  if (r) ctx.fillRect(cx, cy, x + cw - cx, t(r));
  if (u) ctx.fillRect(cx, y, t(u), cy - y + t(u));
  if (d) ctx.fillRect(cx, cy, t(d), y + ch - cy);
  return true;
}

// ---------------------------------------------------------------------------------------------
// The loop and the controls
// ---------------------------------------------------------------------------------------------

function frame(now) {
  requestAnimationFrame(frame);
  if (!loaded || crashed) return;
  const dt = lastTime === null ? 0 : Math.min((now - lastTime) / 1000, 0.25);
  lastTime = now;
  const flags = call(() => wasm.fw_flags());
  if (flags === undefined) return;
  if (flags & 2) {
    const start = performance.now();
    while (performance.now() - start < SEEK_BUDGET) {
      if (!call(() => wasm.fw_seek_step())) break;
    }
  } else {
    call(() => wasm.fw_advance(dt));
  }
  layout();
  draw();
  updateControls();
}

function updateControls() {
  const flags = wasm.fw_flags();
  const turn = wasm.fw_turn();
  const total = wasm.fw_total_turns();
  $('play').textContent = flags & 1 ? 'Play' : 'Pause';
  $('pace').textContent = `×${wasm.fw_pace()}`;
  $('turn').textContent = flags & 2 ? `seeking… ${turn} / ${total}` : `turn ${turn} / ${total}`;
  if (!scrubbing) $('scrub').value = String(turn);
  const shown = Number($('scrub').value);
  $('scrub').style.setProperty('--pct', `${total ? (shown / total) * 100 : 0}%`);
}

function press(code, mods = 0) {
  if (loaded) call(() => wasm.fw_key(code, mods));
}

function keyFromEvent(e) {
  if (e.ctrlKey || e.metaKey) return null; // leave browser shortcuts alone
  const mods = (e.shiftKey ? 1 : 0) | (e.altKey ? 4 : 0);
  if (e.key === 'Tab' && e.shiftKey) return [SPECIAL + 7, 0];
  if (e.key in SPECIAL_KEYS) return [SPECIAL + SPECIAL_KEYS[e.key], mods];
  if ([...e.key].length === 1) return [e.key.codePointAt(0), mods & ~1];
  return null;
}

window.addEventListener('keydown', (e) => {
  if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
  const key = keyFromEvent(e);
  if (!key || !loaded) return;
  e.preventDefault();
  press(...key);
});

$('play').onclick = () => press(' '.codePointAt(0));
$('slower').onclick = () => press('-'.codePointAt(0));
$('faster').onclick = () => press('+'.codePointAt(0));
$('step').onclick = () => press('.'.codePointAt(0));
$('next').onclick = () => press(']'.codePointAt(0));
document.querySelectorAll('.panes button').forEach((b) => {
  b.onclick = () => press(b.dataset.key.codePointAt(0));
});
$('scrub').addEventListener('input', () => { scrubbing = true; });
$('scrub').addEventListener('change', () => {
  scrubbing = false;
  call(() => wasm.fw_seek(Number($('scrub').value)));
});
$('size').onchange = () => { layout(); canvas.focus(); };

$('file').addEventListener('change', async () => {
  const file = $('file').files[0];
  if (file) await openLog(new Uint8Array(await file.arrayBuffer()), file.name);
});
const stage = $('stage');
stage.addEventListener('dragover', (e) => { e.preventDefault(); stage.classList.add('drop'); });
stage.addEventListener('dragleave', () => stage.classList.remove('drop'));
stage.addEventListener('drop', async (e) => {
  e.preventDefault();
  stage.classList.remove('drop');
  const file = e.dataTransfer.files[0];
  if (file) await openLog(new Uint8Array(await file.arrayBuffer()), file.name);
});

// ---------------------------------------------------------------------------------------------
// The timeline's markers
// ---------------------------------------------------------------------------------------------

// What each marker kind (crates/web/src/scan.rs, `MarkerKind::as_str`) is called in the legend,
// in the order it lists them, and the colour it's drawn in (style.css).
const KINDS = {
  death: { label: 'Deaths', color: 'var(--death)' },
  milestone: { label: 'Milestones', color: 'var(--milestone)' },
  avatar: { label: 'Avatar', color: 'var(--avatar)' },
  learned: { label: 'Learned', color: 'var(--learned)' },
  arrival: { label: 'Arrivals', color: 'var(--arrival)' },
  script: { label: 'Script changes', color: 'var(--script)' },
};
const HIDDEN_KEY = 'fn-replay-hidden-markers';

let scanWorker = null;
let markers = []; // { turn, kind, text }, in the order things happened
let scan = { turn: 0, done: false, error: null };
let hiddenKinds = new Set();
try {
  hiddenKinds = new Set(JSON.parse(localStorage.getItem(HIDDEN_KEY) ?? '[]'));
} catch (_) { /* a remembered choice is a convenience */ }

// Replays the whole log once more in a worker to find what to mark, dropping any earlier scan's
// markers; `null` only drops them. The bytes are copied, so the caller may reuse them.
function startScan(bytes) {
  if (scanWorker) scanWorker.terminate();
  scanWorker = null;
  markers = [];
  scan = { turn: 0, done: false, error: null };
  renderMarks();
  if (!bytes || !wasmModule || typeof Worker === 'undefined') return;
  let worker;
  try {
    worker = new Worker(new URL('scan.js', import.meta.url));
  } catch (e) {
    scan.error = String(e);
    renderMarks();
    return;
  }
  scanWorker = worker;
  worker.onmessage = ({ data }) => {
    if (worker !== scanWorker) return;
    if (data.error) {
      scan.error = data.error;
      worker.terminate();
      scanWorker = null;
    } else {
      markers.push(...data.markers);
      scan.turn = data.turn;
      scan.done = data.done;
      if (data.done) {
        worker.terminate();
        scanWorker = null;
      }
    }
    renderMarks();
  };
  worker.onerror = (e) => {
    if (worker !== scanWorker) return;
    scan.error = e.message || 'the scan stopped';
    scanWorker = null;
    renderMarks();
  };
  worker.postMessage({ module: wasmModule, bytes });
}

// Redraws the markers and the legend from `markers`: one mark a kind a turn.
function renderMarks() {
  const total = loaded ? wasm.fw_total_turns() : 0;
  const box = $('marks');
  const nodes = [];
  if (total && !scan.done && !scan.error) {
    const bar = document.createElement('div');
    bar.className = 'scanned';
    bar.style.width = `${(Math.min(scan.turn, total) / total) * 100}%`;
    nodes.push(bar);
  }
  const seen = new Set();
  for (const m of markers) {
    const key = `${m.turn}:${m.kind}`;
    if (!total || hiddenKinds.has(m.kind) || !(m.kind in KINDS) || seen.has(key)) continue;
    seen.add(key);
    const mark = document.createElement('button');
    mark.className = `mark ${m.kind}`;
    mark.tabIndex = -1;
    mark.style.left = `${(Math.min(m.turn, total) / total) * 100}%`;
    mark.dataset.turn = String(m.turn);
    mark.setAttribute('aria-label', `turn ${m.turn}: ${eventsAt(m.turn).map((e) => e.text).join('; ')}`);
    nodes.push(mark);
  }
  box.replaceChildren(...nodes);
  renderLegend();
}

function eventsAt(turn) {
  return markers.filter((m) => m.turn === turn && !hiddenKinds.has(m.kind) && m.kind in KINDS);
}

function renderLegend() {
  const legend = $('legend');
  const counts = {};
  for (const m of markers) counts[m.kind] = (counts[m.kind] ?? 0) + 1;
  const nodes = [];
  for (const [kind, { label, color }] of Object.entries(KINDS)) {
    if (!counts[kind]) continue;
    const chip = document.createElement('button');
    chip.className = hiddenKinds.has(kind) ? 'off' : '';
    chip.title = hiddenKinds.has(kind) ? `show ${label.toLowerCase()}` : `hide ${label.toLowerCase()}`;
    chip.setAttribute('aria-pressed', String(!hiddenKinds.has(kind)));
    const dot = document.createElement('i');
    dot.style.setProperty('--kind', color);
    chip.append(dot, `${label} ${counts[kind]}`);
    chip.onclick = () => {
      if (hiddenKinds.has(kind)) hiddenKinds.delete(kind);
      else hiddenKinds.add(kind);
      try {
        localStorage.setItem(HIDDEN_KEY, JSON.stringify([...hiddenKinds]));
      } catch (_) { /* see above */ }
      renderMarks();
      canvas.focus();
    };
    nodes.push(chip);
  }
  const status = document.createElement('span');
  status.className = 'muted';
  if (scan.error) {
    status.textContent = 'events unavailable';
    status.title = scan.error;
    nodes.push(status);
  } else if (!scan.done) {
    status.textContent = `finding events… ${scan.turn} / ${loaded ? wasm.fw_total_turns() : 0}`;
    nodes.push(status);
  } else if (!markers.length) {
    status.textContent = 'no events to mark';
    nodes.push(status);
  }
  legend.replaceChildren(...nodes);
  legend.hidden = !loaded || !nodes.length;
}

function showTip(mark) {
  const turn = Number(mark.dataset.turn);
  const tip = $('tip');
  const head = document.createElement('b');
  head.textContent = `Turn ${turn}`;
  const lines = eventsAt(turn).map((m) => {
    const line = document.createElement('div');
    const dot = document.createElement('i');
    dot.style.setProperty('--kind', KINDS[m.kind].color);
    line.append(dot, m.text);
    return line;
  });
  tip.replaceChildren(head, ...lines);
  tip.hidden = false;
  // Centred over the mark, kept inside the timeline (and so on screen).
  const box = $('timeline').getBoundingClientRect();
  const at = mark.getBoundingClientRect();
  const width = tip.offsetWidth;
  const x = at.left + at.width / 2 - box.left - width / 2;
  tip.style.left = `${Math.max(0, Math.min(x, box.width - width))}px`;
}

$('marks').addEventListener('mouseover', (e) => {
  const mark = e.target.closest('.mark');
  if (mark) showTip(mark);
});
$('marks').addEventListener('mouseout', (e) => {
  if (e.target.closest('.mark') && !e.relatedTarget?.closest?.('.mark')) $('tip').hidden = true;
});
$('marks').addEventListener('click', (e) => {
  const mark = e.target.closest('.mark');
  if (!mark || !loaded) return;
  call(() => wasm.fw_seek(Number(mark.dataset.turn)));
  canvas.focus();
});

function showMessage(msg, error = false) {
  const el = $('message');
  el.textContent = msg;
  el.classList.toggle('error', error);
}

// The run index (runs/index.json): `cargo xtask web LOG...` writes one for local use; the
// published site has none, since runs are opened from disk there. A list of
// { path, name, nation, seed, build, turns, score? }.
async function loadIndex() {
  let runs;
  try {
    const response = await fetch('runs/index.json');
    if (!response.ok) return;
    runs = (await response.json()).runs ?? [];
  } catch (_) {
    return;
  }
  if (!runs.length) return;
  const select = $('runs');
  select.replaceChildren(new Option('choose a run…', ''));
  for (const run of runs) {
    const bits = [run.nation, run.turns != null ? `${run.turns} turns` : null,
      run.score != null ? `score ${run.score}` : null].filter(Boolean).join(', ');
    select.add(new Option(`${run.name}${bits ? ` (${bits})` : ''}`, run.path));
  }
  select.disabled = false;
  $('pick').hidden = false;
  select.onchange = () => {
    if (!select.value) return;
    const params = new URLSearchParams(location.search);
    params.set('run', select.value);
    history.replaceState(null, '', `?${params}`);
    openUrl(select.value);
  };
}

async function main() {
  try {
    wasm = await loadWasm();
  } catch (e) {
    showMessage(String(e), true);
    return;
  }
  window.addEventListener('resize', () => layout());
  layout();
  await loadIndex();
  const run = new URLSearchParams(location.search).get('run');
  if (run) {
    $('runs').value = run;
    await openUrl(run);
  }
  requestAnimationFrame(frame);
}

main();
