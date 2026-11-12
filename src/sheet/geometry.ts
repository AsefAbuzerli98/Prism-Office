/** Row/column geometry with sparse custom sizes (Excel units → CSS px). */

export const MAX_ROWS = 1_048_576;
export const MAX_COLS = 16_384;
export const DEFAULT_COL_PX = 64; // 8.43 characters of Calibri 11
export const DEFAULT_ROW_PX = 20; // 15pt

/** Excel column width (characters) → px, and back. */
export const colPx = (chars: number) => Math.max(0, Math.round(chars * 7 + 5));
export const colChars = (px: number) => Math.max(0, Math.round(((px - 5) / 7) * 100) / 100);
/** Row height (points) → px, and back. */
export const rowPx = (pt: number) => Math.round((pt * 96) / 72);
export const rowPt = (px: number) => Math.round(((px * 72) / 96) * 100) / 100;

/** One axis: default size plus sorted overrides, O(log n) offset ↔ index. */
export class Axis {
  private idx: number[] = [];
  private size: number[] = [];
  /** prefix[i] = sum of (size - def) for overrides 0..i-1 */
  private prefix: number[] = [0];

  constructor(
    readonly def: number,
    readonly count: number,
    overrides: Iterable<[number, number]> = [],
  ) {
    this.set(overrides);
  }

  set(overrides: Iterable<[number, number]>) {
    const sorted = [...overrides].filter(([i]) => i >= 0 && i < this.count).sort((a, b) => a[0] - b[0]);
    this.idx = sorted.map((x) => x[0]);
    this.size = sorted.map((x) => x[1]);
    this.prefix = [0];
    for (let i = 0; i < sorted.length; i++) this.prefix.push(this.prefix[i]! + this.size[i]! - this.def);
  }

  /** Number of overrides with index < i. */
  private rank(i: number): number {
    let lo = 0;
    let hi = this.idx.length;
    while (lo < hi) {
      const m = (lo + hi) >> 1;
      if (this.idx[m]! < i) lo = m + 1;
      else hi = m;
    }
    return lo;
  }

  sizeOf(i: number): number {
    const k = this.rank(i);
    return this.idx[k] === i ? this.size[k]! : this.def;
  }

  /** Start offset of index i. */
  offset(i: number): number {
    return i * this.def + this.prefix[this.rank(i)]!;
  }

  total(): number {
    return this.offset(this.count);
  }

  /** Index containing offset x (clamped). */
  at(x: number): number {
    if (x <= 0) return 0;
    let lo = 0;
    let hi = this.count - 1;
    while (lo < hi) {
      const m = (lo + hi + 1) >> 1;
      if (this.offset(m) <= x) lo = m;
      else hi = m - 1;
    }
    return lo;
  }
}

/** A1 helpers. */
export function colName(index: number): string {
  let n = index + 1;
  let s = "";
  while (n > 0) {
    const rem = (n - 1) % 26;
    s = String.fromCharCode(65 + rem) + s;
    n = Math.floor((n - 1) / 26);
  }
  return s;
}

export function a1(r: number, c: number): string {
  return `${colName(c)}${r + 1}`;
}

export function parseA1(s: string): { r: number; c: number } | null {
  const m = /^\$?([A-Za-z]{1,3})\$?(\d{1,7})$/.exec(s.trim());
  if (!m) return null;
  let c = 0;
  for (const ch of m[1]!.toUpperCase()) c = c * 26 + ch.charCodeAt(0) - 64;
  const r = Number(m[2]) - 1;
  if (r < 0 || r >= MAX_ROWS || c < 1 || c > MAX_COLS) return null;
  return { r, c: c - 1 };
}

export function parseRange(s: string): { r0: number; c0: number; r1: number; c1: number } | null {
  const [a, b] = s.split(":");
  const p = a ? parseA1(a) : null;
  const q = b ? parseA1(b) : p;
  if (!p || !q) return null;
  return { r0: Math.min(p.r, q.r), c0: Math.min(p.c, q.c), r1: Math.max(p.r, q.r), c1: Math.max(p.c, q.c) };
}

/** The identifier being typed at `pos` in a formula, for autocomplete. */
export function tokenAt(text: string, pos: number): { start: number; word: string } | null {
  if (!text.startsWith("=")) return null;
  // not inside a string literal
  const before = text.slice(0, pos);
  if ((before.match(/"/g) ?? []).length % 2 === 1) return null;
  const m = /([A-Za-z_][A-Za-z0-9_.]*)$/.exec(before);
  if (!m) return null;
  const start = pos - m[1]!.length;
  const prev = text[start - 1];
  // after a sheet name or inside a reference like A1, don't suggest
  if (prev === "!" || prev === "$") return null;
  return { start, word: m[1]! };
}
