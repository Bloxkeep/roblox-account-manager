// The /verify captcha: five characters drawn into a PNG, each tilted, moved and coloured on its own, crossed by wavy lines
// and dots so a script can't simply read it. No library: a 5×7 font as joined round strokes, a PNG written by hand.

// Letters and numbers that can't be mistaken for one another (no 0/O, 1/I/L, 2/Z, 5/S, 6/G, 8/B).
const ALPHABET = 'ACDEFHJKMNPRTUVWXY3479';
const GLYPHS = {
  A: '.###.|#...#|#...#|#####|#...#|#...#|#...#',
  C: '.###.|#...#|#....|#....|#....|#...#|.###.',
  D: '####.|#...#|#...#|#...#|#...#|#...#|####.',
  E: '#####|#....|#....|####.|#....|#....|#####',
  F: '#####|#....|#....|####.|#....|#....|#....',
  H: '#...#|#...#|#...#|#####|#...#|#...#|#...#',
  J: '..###|...#.|...#.|...#.|...#.|#..#.|.##..',
  K: '#...#|#..#.|#.#..|##...|#.#..|#..#.|#...#',
  M: '#...#|##.##|#.#.#|#.#.#|#...#|#...#|#...#',
  N: '#...#|#...#|##..#|#.#.#|#..##|#...#|#...#',
  P: '####.|#...#|#...#|####.|#....|#....|#....',
  R: '####.|#...#|#...#|####.|#.#..|#..#.|#...#',
  T: '#####|..#..|..#..|..#..|..#..|..#..|..#..',
  U: '#...#|#...#|#...#|#...#|#...#|#...#|.###.',
  V: '#...#|#...#|#...#|#...#|#...#|.#.#.|..#..',
  W: '#...#|#...#|#...#|#.#.#|#.#.#|##.##|#...#',
  X: '#...#|#...#|.#.#.|..#..|.#.#.|#...#|#...#',
  Y: '#...#|#...#|.#.#.|..#..|..#..|..#..|..#..',
  3: '####.|....#|....#|.###.|....#|....#|####.',
  4: '...#.|..##.|.#.#.|#..#.|#####|...#.|...#.',
  7: '#####|....#|...#.|..#..|.#...|.#...|.#...',
  9: '.###.|#...#|#...#|.####|....#|...#.|.##..',
};
export const CODE_LENGTH = 5;
const WIDTH = 300, HEIGHT = 100;

/** A random number in [0, 1), from the platform's cryptographic source (a code must not be guessable). */
function random() {
  return crypto.getRandomValues(new Uint32Array(1))[0] / 2 ** 32;
}
const between = (a, b) => a + random() * (b - a);

/** A new code, and its picture as PNG bytes. `fixed` (tests only) is the code to draw instead of a random one. */
export async function captcha(fixed) {
  const code = fixed || Array.from({ length: CODE_LENGTH }, () => ALPHABET[Math.floor(random() * ALPHABET.length)]).join('');
  const rgb = new Uint8Array(WIDTH * HEIGHT * 3);
  for (let i = 0; i < WIDTH * HEIGHT; i++) {
    const shade = 236 + Math.floor(random() * 16);
    rgb.set([shade, shade - Math.floor(random() * 6), shade - Math.floor(random() * 10)], i * 3);
  }
  const dot = (x, y, color, r) => {
    for (let dy = -Math.ceil(r); dy <= r; dy++)
      for (let dx = -Math.ceil(r); dx <= r; dx++) {
        const px = Math.round(x + dx), py = Math.round(y + dy);
        if (dx * dx + dy * dy <= r * r && px >= 0 && py >= 0 && px < WIDTH && py < HEIGHT)
          rgb.set(color, (py * WIDTH + px) * 3);
      }
  };
  const stroke = (x1, y1, x2, y2, color, r) => {
    const steps = Math.max(1, Math.ceil(Math.hypot(x2 - x1, y2 - y1)));
    for (let s = 0; s <= steps; s++)
      dot(x1 + (x2 - x1) * s / steps, y1 + (y2 - y1) * s / steps, color, r);
  };
  const dark = () => [between(10, 110), between(10, 110), between(20, 130)].map(Math.floor);
  const pale = () => [between(120, 180), between(120, 180), between(130, 190)].map(Math.floor);
  const wave = (color, r) => {
    const y0 = between(30, HEIGHT - 30), amp = between(8, 18), freq = between(0.02, 0.045), phase = between(0, 6.3);
    for (let x = 0; x < WIDTH; x++)
      dot(x, y0 + amp * Math.sin(x * freq + phase), color, r);
  };
  wave(pale(), 1.4);
  wave(pale(), 1.4);
  // The letters: the cells of the 5×7 font joined into strokes (sideways, down, and across a corner when nothing else
  // joins them), each letter tilted, moved and coloured on its own.
  const step = WIDTH / (code.length + 1);
  [...code].forEach((ch, k) => {
    const rows = GLYPHS[ch]?.split('|') ?? [];
    const on = (gx, gy) => rows[gy]?.[gx] === '#';
    const color = dark(), angle = between(-0.3, 0.3), cx = step * (k + 1) + between(-4, 4), cy = HEIGHT / 2 + between(-6, 6);
    const sx = between(6.2, 7.2), sy = between(7.6, 8.6), cos = Math.cos(angle), sin = Math.sin(angle);
    const at = (gx, gy) => {
      const lx = (gx - 2) * sx, ly = (gy - 3) * sy;
      return [cx + lx * cos - ly * sin, cy + lx * sin + ly * cos];
    };
    rows.forEach((row, gy) => [...row].forEach((cell, gx) => {
      if (!on(gx, gy))
        return;
      dot(...at(gx, gy), color, 2.6);
      const links = [[1, 0], [0, 1], ...(!on(gx + 1, gy) && !on(gx, gy + 1) ? [[1, 1]] : []), ...(!on(gx - 1, gy) && !on(gx, gy + 1) ? [[-1, 1]] : [])];
      for (const [dx, dy] of links)
        if (on(gx + dx, gy + dy))
          stroke(...at(gx, gy), ...at(gx + dx, gy + dy), color, 2.6);
    }));
  });
  // A thin line in front of the letters, and dots everywhere.
  wave(dark(), 0.9);
  for (let n = 0; n < 600; n++)
    dot(random() * WIDTH, random() * HEIGHT, random() < 0.5 ? dark() : pale(), random() < 0.8 ? 0.5 : 1.2);
  return { code, png: await png(WIDTH, HEIGHT, rgb) };
}

// ── PNG ──

const CRC_TABLE = Uint32Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++)
    c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(bytes) {
  let c = 0xffffffff;
  for (const b of bytes)
    c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const out = new Uint8Array(12 + data.length), view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  out.set(new TextEncoder().encode(type), 4);
  out.set(data, 8);
  view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
  return out;
}

/** RGB pixels (8 bits each) as a PNG file. */
async function png(width, height, rgb) {
  const raw = new Uint8Array((width * 3 + 1) * height); // each row: filter 0 ("none"), then its pixels
  for (let y = 0; y < height; y++)
    raw.set(rgb.subarray(y * width * 3, (y + 1) * width * 3), y * (width * 3 + 1) + 1);
  const zlib = new Uint8Array(await new Response(new Blob([raw]).stream().pipeThrough(new CompressionStream('deflate'))).arrayBuffer());
  const header = new Uint8Array(13), view = new DataView(header.buffer);
  view.setUint32(0, width);
  view.setUint32(4, height);
  header.set([8, 2, 0, 0, 0], 8); // 8 bits, RGB, deflate, no filter method, not interlaced
  const parts = [Uint8Array.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', header), chunk('IDAT', zlib), chunk('IEND', new Uint8Array(0))];
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}
