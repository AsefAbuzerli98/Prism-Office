/**
 * QR code encoder (ISO/IEC 18004): byte mode, error correction L/M/Q/H,
 * versions 1–10, all eight masks with penalty scoring. Enough for the Live
 * Presenting audience link.
 */

export type Ecc = "L" | "M" | "Q" | "H";

// [ec codewords per block, blocks group1, data cw per block g1, blocks g2, data cw per block g2] per version, per level
const TABLE: Record<Ecc, [number, number, number, number, number][]> = {
  L: [
    [7, 1, 19, 0, 0], [10, 1, 34, 0, 0], [15, 1, 55, 0, 0], [20, 1, 80, 0, 0], [26, 1, 108, 0, 0],
    [18, 2, 68, 0, 0], [20, 2, 78, 0, 0], [24, 2, 97, 0, 0], [30, 2, 116, 0, 0], [18, 2, 68, 2, 69],
  ],
  M: [
    [10, 1, 16, 0, 0], [16, 1, 28, 0, 0], [26, 1, 44, 0, 0], [18, 2, 32, 0, 0], [24, 2, 43, 0, 0],
    [16, 4, 27, 0, 0], [18, 4, 31, 0, 0], [22, 2, 38, 2, 39], [22, 3, 36, 2, 37], [26, 4, 43, 1, 44],
  ],
  Q: [
    [13, 1, 13, 0, 0], [22, 1, 22, 0, 0], [18, 2, 17, 0, 0], [26, 2, 24, 0, 0], [18, 2, 15, 2, 16],
    [24, 4, 19, 0, 0], [18, 2, 14, 4, 15], [22, 4, 18, 2, 19], [20, 4, 16, 4, 17], [24, 6, 19, 2, 20],
  ],
  H: [
    [17, 1, 9, 0, 0], [28, 1, 16, 0, 0], [22, 2, 13, 0, 0], [16, 4, 9, 0, 0], [22, 2, 11, 2, 12],
    [28, 4, 15, 0, 0], [26, 4, 13, 1, 14], [26, 4, 14, 2, 15], [24, 4, 12, 4, 13], [28, 6, 15, 2, 16],
  ],
};

const ALIGN: number[][] = [[], [6, 18], [6, 22], [6, 26], [6, 30], [6, 34], [6, 22, 38], [6, 24, 42], [6, 26, 46], [6, 28, 50]];
const ECC_BITS: Record<Ecc, number> = { L: 1, M: 0, Q: 3, H: 2 };

// ---- Reed-Solomon over GF(256), polynomial 0x11d ---------------------------------------------

const EXP = new Uint8Array(512);
const LOG = new Uint8Array(256);
{
  let x = 1;
  for (let i = 0; i < 255; i++) {
    EXP[i] = x;
    LOG[x] = i;
    x <<= 1;
    if (x & 0x100) x ^= 0x11d;
  }
  for (let i = 255; i < 512; i++) EXP[i] = EXP[i - 255]!;
}
const mul = (a: number, b: number) => (a && b ? EXP[LOG[a]! + LOG[b]!]! : 0);

function generator(n: number): number[] {
  let g = [1];
  for (let i = 0; i < n; i++) {
    const next = new Array(g.length + 1).fill(0);
    for (let j = 0; j < g.length; j++) {
      next[j] ^= g[j]!;
      next[j + 1] ^= mul(g[j]!, EXP[i]!);
    }
    g = next;
  }
  return g;
}

/** Error-correction codewords for a data block. */
export function rsEncode(data: number[], ecLen: number): number[] {
  const g = generator(ecLen);
  const rem = new Array(ecLen).fill(0);
  for (const d of data) {
    const f = d ^ rem.shift()!;
    rem.push(0);
    for (let i = 0; i < ecLen; i++) rem[i] ^= mul(g[i + 1]!, f);
  }
  return rem;
}

/** 15-bit format string (ECC level + mask, BCH, XOR mask). */
export function formatBits(ecc: Ecc, mask: number): number {
  const data = (ECC_BITS[ecc] << 3) | mask;
  let rem = data << 10;
  for (let i = 14; i >= 10; i--) if (rem & (1 << i)) rem ^= 0x537 << (i - 10);
  return ((data << 10) | rem) ^ 0x5412;
}

function versionBits(v: number): number {
  let rem = v << 12;
  for (let i = 17; i >= 12; i--) if (rem & (1 << i)) rem ^= 0x1f25 << (i - 12);
  return (v << 12) | rem;
}

// ---- encoding ------------------------------------------------------------------------------------

function dataCapacity(v: number, ecc: Ecc) {
  const [, b1, d1, b2, d2] = TABLE[ecc][v - 1]!;
  return b1 * d1 + b2 * d2;
}

function codewords(bytes: Uint8Array, v: number, ecc: Ecc): number[] {
  const cap = dataCapacity(v, ecc);
  const bits: number[] = [];
  const put = (val: number, n: number) => {
    for (let i = n - 1; i >= 0; i--) bits.push((val >> i) & 1);
  };
  put(0b0100, 4);
  put(bytes.length, v < 10 ? 8 : 16);
  for (const b of bytes) put(b, 8);
  put(0, Math.min(4, cap * 8 - bits.length));
  while (bits.length % 8) bits.push(0);
  const data: number[] = [];
  for (let i = 0; i < bits.length; i += 8) data.push(bits.slice(i, i + 8).reduce((a, b) => (a << 1) | b, 0));
  for (let p = 0; data.length < cap; p++) data.push(p % 2 ? 0x11 : 0xec);
  // blocks + interleave
  const [ecLen, b1, d1, b2, d2] = TABLE[ecc][v - 1]!;
  const blocks: number[][] = [];
  let k = 0;
  for (let i = 0; i < b1; i++) (blocks.push(data.slice(k, k + d1)), (k += d1));
  for (let i = 0; i < b2; i++) (blocks.push(data.slice(k, k + d2)), (k += d2));
  const ecs = blocks.map((b) => rsEncode(b, ecLen));
  const out: number[] = [];
  for (let i = 0; i < Math.max(d1, d2); i++) for (const b of blocks) if (i < b.length) out.push(b[i]!);
  for (let i = 0; i < ecLen; i++) for (const e of ecs) out.push(e[i]!);
  return out;
}

export interface QrCode {
  version: number;
  size: number;
  mask: number;
  /** modules[y][x] true = dark */
  modules: boolean[][];
}

/** Function-pattern modules (not data). */
function reservedMap(v: number): boolean[][] {
  const n = 17 + 4 * v;
  const r = Array.from({ length: n }, () => new Array<boolean>(n).fill(false));
  const rect = (x0: number, y0: number, w: number, hh: number) => {
    for (let y = Math.max(0, y0); y < Math.min(n, y0 + hh); y++) for (let x = Math.max(0, x0); x < Math.min(n, x0 + w); x++) r[y]![x] = true;
  };
  rect(0, 0, 9, 9);
  rect(n - 8, 0, 8, 9);
  rect(0, n - 8, 9, 8);
  rect(6, 0, 1, n);
  rect(0, 6, n, 1);
  const al = ALIGN[v - 1]!;
  for (const ay of al) for (const ax of al) if (!(ax <= 8 && ay <= 8) && !(ax >= n - 9 && ay <= 8) && !(ax <= 8 && ay >= n - 9)) rect(ax - 2, ay - 2, 5, 5);
  if (v >= 7) {
    rect(n - 11, 0, 3, 6);
    rect(0, n - 11, 6, 3);
  }
  return r;
}

const MASKS: ((x: number, y: number) => boolean)[] = [
  (x, y) => (x + y) % 2 === 0,
  (_x, y) => y % 2 === 0,
  (x) => x % 3 === 0,
  (x, y) => (x + y) % 3 === 0,
  (x, y) => (Math.floor(y / 2) + Math.floor(x / 3)) % 2 === 0,
  (x, y) => ((x * y) % 2) + ((x * y) % 3) === 0,
  (x, y) => (((x * y) % 2) + ((x * y) % 3)) % 2 === 0,
  (x, y) => (((x + y) % 2) + ((x * y) % 3)) % 2 === 0,
];

/** Zig-zag data module order (right-to-left column pairs, alternating up/down). */
export function dataOrder(v: number): [number, number][] {
  const n = 17 + 4 * v;
  const res = reservedMap(v);
  const out: [number, number][] = [];
  let up = true;
  for (let x = n - 1; x > 0; x -= 2) {
    if (x === 6) x = 5;
    for (let i = 0; i < n; i++) {
      const y = up ? n - 1 - i : i;
      for (const dx of [0, 1]) if (!res[y]![x - dx]) out.push([x - dx, y]);
    }
    up = !up;
  }
  return out;
}

function drawFunction(m: boolean[][], v: number) {
  const n = m.length;
  const finder = (cx: number, cy: number) => {
    for (let dy = -4; dy <= 4; dy++)
      for (let dx = -4; dx <= 4; dx++) {
        const x = cx + dx;
        const y = cy + dy;
        if (x < 0 || y < 0 || x >= n || y >= n) continue;
        const d = Math.max(Math.abs(dx), Math.abs(dy));
        m[y]![x] = d !== 2 && d !== 4;
      }
  };
  finder(3, 3);
  finder(n - 4, 3);
  finder(3, n - 4);
  for (let i = 8; i < n - 8; i++) m[6]![i] = m[i]![6] = i % 2 === 0;
  const al = ALIGN[v - 1]!;
  for (const ay of al)
    for (const ax of al) {
      if ((ax <= 8 && ay <= 8) || (ax >= n - 9 && ay <= 8) || (ax <= 8 && ay >= n - 9)) continue;
      for (let dy = -2; dy <= 2; dy++) for (let dx = -2; dx <= 2; dx++) m[ay + dy]![ax + dx] = Math.max(Math.abs(dx), Math.abs(dy)) !== 1;
    }
  m[n - 8]![8] = true; // dark module
  if (v >= 7) {
    const vb = versionBits(v);
    for (let i = 0; i < 18; i++) {
      const bit = ((vb >> i) & 1) === 1;
      const a = Math.floor(i / 3);
      const b = (i % 3) + n - 11;
      m[a]![b] = bit;
      m[b]![a] = bit;
    }
  }
}

function drawFormat(m: boolean[][], ecc: Ecc, mask: number) {
  const n = m.length;
  const f = formatBits(ecc, mask);
  const bit = (i: number) => ((f >> i) & 1) === 1;
  // around the top-left finder
  for (let i = 0; i <= 5; i++) m[i]![8] = bit(i);
  m[7]![8] = bit(6);
  m[8]![8] = bit(7);
  m[8]![7] = bit(8);
  for (let i = 9; i < 15; i++) m[8]![14 - i] = bit(i);
  // split copy
  for (let i = 0; i < 8; i++) m[8]![n - 1 - i] = bit(i);
  for (let i = 8; i < 15; i++) m[n - 15 + i]![8] = bit(i);
}

function penalty(m: boolean[][]): number {
  const n = m.length;
  let p = 0;
  for (let pass = 0; pass < 2; pass++)
    for (let a = 0; a < n; a++) {
      let run = 1;
      for (let b = 1; b < n; b++) {
        const cur = pass ? m[b]![a] : m[a]![b];
        const prev = pass ? m[b - 1]![a] : m[a]![b - 1];
        if (cur === prev) run++;
        else {
          if (run >= 5) p += run - 2;
          run = 1;
        }
      }
      if (run >= 5) p += run - 2;
    }
  for (let y = 0; y < n - 1; y++) for (let x = 0; x < n - 1; x++) if (m[y]![x] === m[y]![x + 1] && m[y]![x] === m[y + 1]![x] && m[y]![x] === m[y + 1]![x + 1]) p += 3;
  const pat = [true, false, true, true, true, false, true, false, false, false, false];
  const rev = [...pat].reverse();
  for (let y = 0; y < n; y++)
    for (let x = 0; x + 11 <= n; x++)
      for (const pt of [pat, rev]) {
        if (pt.every((v, i) => m[y]![x + i] === v)) p += 40;
        if (pt.every((v, i) => m[x + i]![y] === v)) p += 40;
      }
  let dark = 0;
  for (const row of m) for (const v of row) if (v) dark++;
  p += Math.floor(Math.abs((dark * 100) / (n * n) - 50) / 5) * 10;
  return p;
}

/** Encode text (UTF-8, byte mode) as a QR code. */
export function encodeQr(text: string, ecc: Ecc = "M"): QrCode {
  const bytes = new TextEncoder().encode(text);
  let v = 1;
  while (v <= 10 && dataCapacity(v, ecc) < bytes.length + 2 + (v >= 10 ? 1 : 0)) v++;
  if (v > 10) throw new Error("Text too long for a QR code (version 10 max)");
  const cws = codewords(bytes, v, ecc);
  const n = 17 + 4 * v;
  const order = dataOrder(v);
  let best: QrCode | null = null;
  let bestScore = Infinity;
  for (let mask = 0; mask < 8; mask++) {
    const m = Array.from({ length: n }, () => new Array<boolean>(n).fill(false));
    drawFunction(m, v);
    order.forEach(([x, y], i) => {
      const bit = i < cws.length * 8 ? ((cws[i >> 3]! >> (7 - (i & 7))) & 1) === 1 : false;
      m[y]![x] = bit !== MASKS[mask]!(x, y);
    });
    drawFormat(m, ecc, mask);
    const score = penalty(m);
    if (score < bestScore) {
      bestScore = score;
      best = { version: v, size: n, mask, modules: m };
    }
  }
  return best!;
}

/** Draw a QR code with a quiet zone into a canvas context. */
export function drawQr(ctx: CanvasRenderingContext2D, qr: QrCode, x: number, y: number, size: number, dark = "#000", light = "#fff") {
  const cells = qr.size + 8;
  const s = size / cells;
  ctx.fillStyle = light;
  ctx.fillRect(x, y, size, size);
  ctx.fillStyle = dark;
  for (let r = 0; r < qr.size; r++) for (let c = 0; c < qr.size; c++) if (qr.modules[r]![c]) ctx.fillRect(x + (c + 4) * s, y + (r + 4) * s, Math.ceil(s), Math.ceil(s));
}

/** SVG markup of a QR code (crisp at any size). */
export function qrSvg(qr: QrCode, px = 256): string {
  const cells = qr.size + 8;
  let d = "";
  for (let r = 0; r < qr.size; r++) for (let c = 0; c < qr.size; c++) if (qr.modules[r]![c]) d += `M${c + 4} ${r + 4}h1v1h-1z`;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${px}" height="${px}" viewBox="0 0 ${cells} ${cells}" shape-rendering="crispEdges"><rect width="100%" height="100%" fill="#fff"/><path d="${d}" fill="#000"/></svg>`;
}
