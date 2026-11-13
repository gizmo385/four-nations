// The offline copy of the game (#974, docs/design/touch-pwa.md §1). `cargo xtask web-gui`
// writes the build's hash in for __FN_BUILD__, so each build is a new worker with its own cache,
// and the page asks for `fn_gui.js?v=<hash>` and `fn_gui_bg.wasm?v=<hash>`.
//
// - The page (`./`, `index.html`) is network first: online, a new build shows at once; offline,
//   the cached page.
// - Everything else (the hashed glue and wasm, the icons, the manifest) is cache first: a
//   build's files never change under their names.
//
// Saves are in localStorage, not here, so a new build never touches them.

const BUILD = '__FN_BUILD__';
const CACHE = `fn-play-${BUILD}`;
const FILES = [
  './',
  `fn_gui.js?v=${BUILD}`,
  `fn_gui_bg.wasm?v=${BUILD}`,
  'manifest.webmanifest',
  'icons/icon-180.png',
  'icons/icon-192.png',
  'icons/icon-512.png',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((cache) => cache.addAll(FILES))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((names) =>
        Promise.all(
          names.filter((n) => n.startsWith('fn-play-') && n !== CACHE).map((n) => caches.delete(n)),
        ),
      )
      .then(() => self.clients.claim()),
  );
});

function isPage(request) {
  return request.mode === 'navigate' || new URL(request.url).pathname.endsWith('/index.html');
}

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET' || new URL(request.url).origin !== self.location.origin) return;
  if (isPage(request)) {
    event.respondWith(
      fetch(request)
        .then((response) => {
          if (response.ok) {
            const copy = response.clone();
            caches.open(CACHE).then((cache) => cache.put('./', copy));
          }
          return response;
        })
        .catch(() => caches.match('./', { cacheName: CACHE }).then((r) => r || caches.match('./'))),
    );
    return;
  }
  event.respondWith(caches.match(request).then((hit) => hit || fetch(request)));
});
