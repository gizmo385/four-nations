// The timeline's scan (crates/web/src/scan.rs), in a worker: a second instance of fn_web.wasm
// replays the whole log once more, apart from the one being watched, to find the turns worth
// marking on the scrub bar. The page (replay.js) sends { module, bytes }; this posts back
// { markers, turn, done } as it goes, each batch holding only markers it hasn't sent yet, or
// { error } if the scan can't run.

// How often to send what's been found, in ms.
const BATCH_MS = 250;

self.onmessage = async ({ data }) => {
  let wasm;
  try {
    wasm = (await WebAssembly.instantiate(data.module, {})).exports;
  } catch (e) {
    self.postMessage({ error: String(e) });
    return;
  }
  const text = () =>
    new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, wasm.fw_text_ptr(), wasm.fw_text_len()));
  let sent = 0;
  const flush = (done) => {
    const count = wasm.fw_marker_count();
    const markers = [];
    if (count > sent) {
      wasm.fw_markers(sent);
      for (const line of text().split('\n')) {
        if (!line) continue;
        const [turn, kind, ...rest] = line.split('\t');
        markers.push({ turn: Number(turn), kind, text: rest.join('\t') });
      }
      sent = count;
    }
    self.postMessage({ markers, turn: wasm.fw_scan_turn(), done });
  };
  try {
    const { bytes } = data;
    const ptr = wasm.fw_input(bytes.length);
    new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
    if (!wasm.fw_scan_open()) {
      self.postMessage({ error: text() });
      return;
    }
    flush(false);
    let last = performance.now();
    while (wasm.fw_scan_step()) {
      const now = performance.now();
      if (now - last >= BATCH_MS) {
        flush(false);
        last = now;
      }
    }
    flush(true);
  } catch (e) {
    let why = String(e);
    try {
      if (wasm.fw_panic() > 0) why = text();
    } catch (_) { /* the panic message is best effort */ }
    self.postMessage({ error: why });
  }
};
