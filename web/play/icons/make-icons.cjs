// Renders icon.txt (the 16-pixel app icon, #974) to the PNGs the manifest and the iOS
// apple-touch-icon name. Run after changing icon.txt and commit the PNGs:
//
//   node web/play/icons/make-icons.cjs
//
// Each pixel becomes a square block (nearest neighbour), centred on the background colour. The
// maskable one leaves a wide margin so the ring stays inside the 80% circle a launcher may cut
// it to. Node's own zlib only, nothing to install.

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const dir = __dirname;
const SIZE = 16;
// [file, image size, block size]: the art is SIZE * block pixels, centred.
const OUTPUTS = [
  ['icon-180.png', 180, 11],
  ['icon-192.png', 192, 12],
  ['icon-512.png', 512, 32],
  ['icon-maskable-512.png', 512, 24],
];

function parse(text) {
  const palette = {};
  const grid = [];
  for (const line of text.split('\n')) {
    if (line.startsWith('#') || line.trim() === '') continue;
    const m = line.match(/^(\S) = #([0-9a-f]{6})$/);
    if (m) {
      palette[m[1]] = [0, 2, 4].map((i) => parseInt(m[2].slice(i, i + 2), 16));
    } else {
      grid.push(line);
    }
  }
  if (grid.length !== SIZE || grid.some((r) => r.length !== SIZE)) {
    throw new Error(`icon.txt: the grid should be ${SIZE} rows of ${SIZE}`);
  }
  for (const row of grid) {
    for (const c of row) {
      if (!palette[c]) throw new Error(`icon.txt: ${JSON.stringify(c)} is not in the palette`);
    }
  }
  return { palette, grid, background: palette['.'] };
}

const CRC = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = CRC[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

// An RGB PNG of `size` square from `pixel(x, y) -> [r, g, b]`.
function png(size, pixel) {
  const raw = Buffer.alloc(size * (size * 3 + 1));
  for (let y = 0; y < size; y++) {
    const row = y * (size * 3 + 1);
    raw[row] = 0; // filter: none
    for (let x = 0; x < size; x++) {
      const [r, g, b] = pixel(x, y);
      raw.set([r, g, b], row + 1 + x * 3);
    }
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(size, 0);
  header.writeUInt32BE(size, 4);
  header.set([8, 2, 0, 0, 0], 8); // 8-bit, RGB, deflate, no filter set, no interlace
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

const icon = parse(fs.readFileSync(path.join(dir, 'icon.txt'), 'utf8'));
for (const [file, size, block] of OUTPUTS) {
  const pad = Math.floor((size - SIZE * block) / 2);
  const out = png(size, (x, y) => {
    const gx = Math.floor((x - pad) / block);
    const gy = Math.floor((y - pad) / block);
    if (x < pad || y < pad || gx >= SIZE || gy >= SIZE) return icon.background;
    return icon.palette[icon.grid[gy][gx]];
  });
  fs.writeFileSync(path.join(dir, file), out);
  console.log(`${file}: ${size}x${size}, ${out.length} bytes`);
}
