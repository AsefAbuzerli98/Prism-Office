/**
 * Paragraph layout & pagination (P2-04, P2-06).
 *
 * Coordinates are CSS pixels; OOXML twips convert at 15 twips/px (96 dpi).
 * Lines are built from unbreakable clusters (words incl. trailing spaces,
 * CJK characters, tabs, opaque objects) placed greedily; justification
 * spreads slack across cluster gaps. Pagination honours page breaks,
 * page-break-before, keep-with-next and single-line orphan control.
 */
import { isParagraph, type Align, type Block, type Marks, type NumberingDef, type ParaFormat, type Paragraph, type StyleInfo, type WordDocument } from "./model";
import { inlineLength } from "./ops";

export interface Measurer {
  width(text: string, font: string): number;
}

export const twipsToPx = (t: number) => t / 15;
const halfPtToPx = (hp: number) => (hp / 2) * (96 / 72);

export interface RunStyle {
  font: string;
  family: string;
  sizePx: number;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
  color: string;
  highlight: string | null;
  vert: "superscript" | "subscript" | null;
  link: string | null;
}

export interface Frag {
  kind: "text" | "tab" | "opaque" | "marker";
  x: number;
  width: number;
  text: string;
  style: RunStyle;
  /** Paragraph offsets covered [start, end). Markers cover none. */
  start: number;
  end: number;
}

export interface Line {
  block: number;
  /** Page-relative top. */
  y: number;
  height: number;
  baseline: number;
  /** Content box left/right (page-relative) for this line. */
  left: number;
  right: number;
  frags: Frag[];
  start: number;
  end: number;
  /** Last line of its paragraph. */
  last: boolean;
}

export interface BlockBox {
  block: number;
  y: number;
  height: number;
  left: number;
  right: number;
  label: string;
}

export interface Page {
  index: number;
  lines: Line[];
  boxes: BlockBox[];
}

export interface Layout {
  pages: Page[];
  width: number;
  height: number;
  margin: { top: number; right: number; bottom: number; left: number };
}

const TAB_PX = 48; // default 0.5" tab stops
const OPAQUE_BLOCK_PX = 44;
const HIGHLIGHTS: Record<string, string> = {
  yellow: "#fff59d", green: "#a5d6a7", cyan: "#80deea", magenta: "#f48fb1", blue: "#90caf9", red: "#ef9a9a",
  darkBlue: "#5c6bc0", darkCyan: "#4db6ac", darkGreen: "#66bb6a", darkMagenta: "#ab47bc", darkRed: "#e57373",
  darkYellow: "#d4c15a", darkGray: "#9e9e9e", lightGray: "#e0e0e0", black: "#424242",
};

export function resolveStyle(doc: WordDocument, id: string | undefined): StyleInfo | undefined {
  if (id) {
    const s = doc.styles.find((x) => x.id === id);
    if (s) return s;
  }
  return doc.styles.find((s) => s.kind === "paragraph" && s.isDefault);
}

export function effectiveFormat(doc: WordDocument, p: Paragraph): ParaFormat {
  const style = resolveStyle(doc, p.style);
  const fmt: ParaFormat = { ...doc.defaultPara, ...stripUndef(style?.para ?? {}) };
  const direct: ParaFormat = {
    align: p.align, spaceBefore: p.spaceBefore, spaceAfter: p.spaceAfter, line: p.line, indentLeft: p.indentLeft,
    indentRight: p.indentRight, indentFirst: p.indentFirst, keepNext: p.keepNext, pageBreakBefore: p.pageBreakBefore,
  };
  Object.assign(fmt, stripUndef(direct));
  if (p.list && p.indentLeft === undefined) {
    const lvl = listLevel(doc.numbering, p.list.numId, p.list.level);
    if (lvl) {
      fmt.indentLeft = lvl.indentLeft;
      if (p.indentFirst === undefined) fmt.indentFirst = -lvl.hanging;
    }
  }
  return fmt;
}

function stripUndef<T extends object>(o: T): Partial<T> {
  return Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined)) as Partial<T>;
}

export function effectiveMarks(doc: WordDocument, p: Paragraph, m: Marks): Marks {
  const style = resolveStyle(doc, p.style);
  const charStyle = m.style ? doc.styles.find((s) => s.id === m.style)?.run : undefined;
  return { ...doc.defaultRun, ...stripUndef(style?.run ?? {}), ...stripUndef(charStyle ?? {}), ...stripUndef(m) };
}

export function runStyle(m: Marks): RunStyle {
  let sizePx = halfPtToPx(m.size ?? 22);
  const vert = m.vertAlign ?? null;
  if (vert) sizePx *= 0.65;
  const family = m.font ?? "Calibri";
  const stack = `"${family}", ${/mono|consolas|courier/i.test(family) ? "ui-monospace, monospace" : "Carlito, system-ui, sans-serif"}`;
  const font = `${m.italic ? "italic " : ""}${m.bold ? "700 " : "400 "}${sizePx.toFixed(2)}px ${stack}`;
  return {
    font,
    family,
    sizePx,
    bold: !!m.bold,
    italic: !!m.italic,
    underline: !!m.underline || !!m.link,
    strike: !!m.strike,
    color: m.link && !m.color ? "#1d4ed8" : `#${m.color ?? "1f2328"}`,
    highlight: m.highlight ? (HIGHLIGHTS[m.highlight] ?? null) : null,
    vert,
    link: m.link ?? null,
  };
}

function listLevel(defs: NumberingDef[], numId: number, level: number) {
  return defs.find((d) => d.numId === numId)?.levels[level];
}

function toRoman(n: number): string {
  const map: [number, string][] = [[1000, "m"], [900, "cm"], [500, "d"], [400, "cd"], [100, "c"], [90, "xc"], [50, "l"], [40, "xl"], [10, "x"], [9, "ix"], [5, "v"], [4, "iv"], [1, "i"]];
  let s = "";
  for (const [v, r] of map) while (n >= v) { s += r; n -= v; }
  return s;
}

function toLetters(n: number): string {
  let s = "";
  while (n > 0) { s = String.fromCharCode(97 + ((n - 1) % 26)) + s; n = Math.floor((n - 1) / 26); }
  return s;
}

export function formatNumber(n: number, format: string): string {
  switch (format) {
    case "lowerLetter": return toLetters(n);
    case "upperLetter": return toLetters(n).toUpperCase();
    case "lowerRoman": return toRoman(n);
    case "upperRoman": return toRoman(n).toUpperCase();
    case "bullet": return "";
    default: return String(n);
  }
}

/** Compute marker text for every list paragraph (counters per numId/level). */
export function listMarkers(doc: WordDocument): Map<number, string> {
  const out = new Map<number, string>();
  const counters = new Map<number, number[]>();
  doc.blocks.forEach((b, i) => {
    if (!isParagraph(b) || !b.list) return;
    const { numId, level } = b.list;
    const def = doc.numbering.find((d) => d.numId === numId);
    const lvl = def?.levels[level];
    if (!def || !lvl) return;
    const c = counters.get(numId) ?? [];
    c[level] = (c[level] ?? (lvl.start - 1)) + 1;
    c.length = level + 1; // reset deeper levels
    counters.set(numId, c);
    if (lvl.format === "bullet") {
      out.set(i, lvl.text.replace("", "•") || "•");
      return;
    }
    out.set(i, lvl.text.replace(/%(\d)/g, (_, d: string) => {
      const L = Number(d) - 1;
      const ld = def.levels[L];
      return formatNumber(c[L] ?? ld?.start ?? 1, ld?.format ?? "decimal");
    }));
  });
  return out;
}

// ---------------------------------------------------------------------------

interface Item {
  kind: "text" | "tab" | "opaque" | "break";
  text: string;
  style: RunStyle;
  start: number;
  end: number;
  width: number;
  /** Width of trailing whitespace (hangs at line end). */
  trailing: number;
  breakAfter: boolean;
  breakKind?: "line" | "page" | "column";
}

const CJK = /[⺀-鿿가-힯豈-﫿　-〿＀-￯]/;

/** Split text into clusters with break opportunities after whitespace, hyphens and around CJK. */
export function tokenize(text: string): string[] {
  const out: string[] = [];
  let cur = "";
  const chars = Array.from(text);
  for (let i = 0; i < chars.length; i++) {
    const c = chars[i]!;
    const next = chars[i + 1];
    if (CJK.test(c)) {
      if (cur) out.push(cur);
      cur = c;
      if (next && /\s/.test(next)) continue;
      out.push(cur);
      cur = "";
      continue;
    }
    cur += c;
    const isBreakChar = /\s/.test(c) ? next !== undefined && !/\s/.test(next) : (c === "-" || c === "—" || c === "/") && next !== undefined && !/\s/.test(next) && cur.length > 1;
    if (isBreakChar) {
      out.push(cur);
      cur = "";
    }
  }
  if (cur) out.push(cur);
  return out;
}

function buildItems(doc: WordDocument, p: Paragraph, m: Measurer): Item[] {
  const items: Item[] = [];
  let offset = 0;
  for (const inl of p.inlines) {
    const len = inlineLength(inl);
    if (inl.type === "text") {
      const style = runStyle(effectiveMarks(doc, p, inl.marks));
      let o = offset;
      for (const tok of tokenize(inl.text)) {
        const trailingText = tok.match(/\s+$/)?.[0] ?? "";
        const w = m.width(tok, style.font);
        const lastChar = tok[tok.length - 1] ?? "";
        items.push({
          kind: "text",
          text: tok,
          style,
          start: o,
          end: o + tok.length,
          width: w,
          trailing: trailingText ? m.width(trailingText, style.font) : 0,
          breakAfter: /[\s\-—/]/.test(lastChar) || CJK.test(lastChar),
        });
        o += tok.length;
      }
    } else if (inl.type === "tab") {
      const style = runStyle(effectiveMarks(doc, p, inl.marks));
      items.push({ kind: "tab", text: "\t", style, start: offset, end: offset + 1, width: 0, trailing: 0, breakAfter: true });
    } else if (inl.type === "break") {
      const style = runStyle(effectiveMarks(doc, p, {}));
      items.push({ kind: "break", text: "", style, start: offset, end: offset + 1, width: 0, trailing: 0, breakAfter: true, breakKind: inl.kind });
    } else {
      const style = runStyle(effectiveMarks(doc, p, {}));
      const label = inl.label ? `⟦${inl.label}⟧` : "";
      const w = label ? m.width(label, style.font) + 6 : 0;
      items.push({ kind: "opaque", text: label, style, start: offset, end: offset + 1, width: w, trailing: 0, breakAfter: true });
    }
    offset += len;
  }
  return items;
}

interface RawLine {
  frags: Frag[];
  start: number;
  end: number;
  height: number;
  baseline: number;
  width: number;
  forced: boolean;
  pageBreakAfter: boolean;
}

function lineMetrics(frags: Frag[], fallback: RunStyle, factor: number) {
  const sizes = frags.length ? frags.map((f) => (f.style.vert ? f.style.sizePx / 0.65 : f.style.sizePx)) : [fallback.sizePx];
  const max = Math.max(...sizes);
  const natural = max * 1.22;
  const height = natural * factor;
  return { height, baseline: (height - natural) / 2 + max * 0.98 };
}

/** Break one paragraph into lines for a given available width. */
export function breakParagraph(doc: WordDocument, p: Paragraph, m: Measurer, avail: number, firstIndent: number): RawLine[] {
  const fmt = effectiveFormat(doc, p);
  const factor = Math.max(0.5, (fmt.line ?? 240) / 240);
  const items = buildItems(doc, p, m);
  const fallback = runStyle(effectiveMarks(doc, p, {}));
  const lines: RawLine[] = [];
  let frags: Frag[] = [];
  let x = firstIndent;
  let lineStart = 0;
  let lineAvail = avail;

  const finish = (end: number, forced: boolean, pageBreakAfter = false) => {
    const last = frags[frags.length - 1];
    const trailing = last?.kind === "text" ? m.width(last.text.match(/\s+$/)?.[0] ?? "", last.style.font) : 0;
    const width = (last ? last.x + last.width - trailing : x) - (lines.length === 0 ? firstIndent : 0);
    const met = lineMetrics(frags, fallback, factor);
    lines.push({ frags, start: lineStart, end, height: met.height, baseline: met.baseline, width, forced, pageBreakAfter });
    frags = [];
    x = 0;
    lineStart = end;
    lineAvail = avail;
  };

  let i = 0;
  while (i < items.length) {
    const it = items[i]!;
    if (it.kind === "break") {
      frags.push({ kind: "text", x, width: 0, text: "", style: it.style, start: it.start, end: it.end });
      finish(it.end, true, it.breakKind === "page" || it.breakKind === "column");
      i++;
      continue;
    }
    if (it.kind === "tab") {
      const w = Math.max(4, (Math.floor(x / TAB_PX) + 1) * TAB_PX - x);
      if (x + w > lineAvail && frags.length) finish(it.start, false);
      const tw = Math.max(4, (Math.floor(x / TAB_PX) + 1) * TAB_PX - x);
      frags.push({ kind: "tab", x, width: tw, text: "", style: it.style, start: it.start, end: it.end });
      x += tw;
      i++;
      continue;
    }
    // Gather an unbreakable cluster (items glued across run boundaries).
    let j = i;
    let clusterW = 0;
    while (j < items.length) {
      const c = items[j]!;
      if (c.kind === "break" || c.kind === "tab") break;
      clusterW += c.width;
      j++;
      if (c.breakAfter) break;
    }
    const cluster = items.slice(i, j);
    const trailing = cluster[cluster.length - 1]!.trailing;
    if (x + clusterW - trailing > lineAvail + 0.01 && frags.length > 0) {
      finish(cluster[0]!.start, false);
    }
    if (x + clusterW - trailing > lineAvail + 0.01 && frags.length === 0) {
      // Emergency break: the cluster alone is wider than the line.
      for (const c of cluster) {
        let rest = c.text;
        let start = c.start;
        while (rest) {
          let n = rest.length;
          while (n > 1 && x + m.width(rest.slice(0, n), c.style.font) > lineAvail) n--;
          const piece = rest.slice(0, n);
          const w = m.width(piece, c.style.font);
          if (x + w > lineAvail + 0.01 && frags.length) {
            finish(start, false);
            continue;
          }
          frags.push({ kind: c.kind === "opaque" ? "opaque" : "text", x, width: w, text: piece, style: c.style, start, end: start + piece.length });
          x += w;
          start += piece.length;
          rest = rest.slice(n);
          if (rest) finish(start, false);
        }
      }
      i = j;
      continue;
    }
    for (const c of cluster) {
      frags.push({ kind: c.kind === "opaque" ? "opaque" : "text", x, width: c.width, text: c.text, style: c.style, start: c.start, end: c.end });
      x += c.width;
    }
    i = j;
  }
  const total = p.inlines.reduce((n, inl) => n + inlineLength(inl), 0);
  if (frags.length || lines.length === 0 || lines[lines.length - 1]!.forced) finish(total, true);
  return lines;
}

function alignLine(line: RawLine, avail: number, align: Align | undefined, isLast: boolean, indent: number) {
  const slack = avail - indent - line.width;
  if (slack <= 0) return;
  if (align === "center" || align === "right") {
    const dx = align === "center" ? slack / 2 : slack;
    for (const f of line.frags) f.x += dx;
  } else if (align === "justify" && !isLast && !line.forced) {
    const gaps = line.frags.slice(0, -1).filter((f) => f.kind === "text" && /\s$/.test(f.text)).length;
    if (!gaps) return;
    const per = slack / gaps;
    let shift = 0;
    for (const f of line.frags) {
      f.x += shift;
      if (f.kind === "text" && /\s$/.test(f.text) && f !== line.frags[line.frags.length - 1]) shift += per;
    }
  }
}

interface Cached {
  key: string;
  lines: RawLine[];
}
const cache = new WeakMap<Paragraph, Cached>();

function cloneLines(lines: RawLine[]): RawLine[] {
  return lines.map((l) => ({ ...l, frags: l.frags.map((f) => ({ ...f })) }));
}

/** Lay out and paginate the whole document. */
export function layoutDocument(doc: WordDocument, m: Measurer): Layout {
  const s = doc.section;
  const width = twipsToPx(s.pageWidth);
  const height = twipsToPx(s.pageHeight);
  const margin = {
    top: twipsToPx(Math.abs(s.marginTop)),
    right: twipsToPx(s.marginRight),
    bottom: twipsToPx(Math.abs(s.marginBottom)),
    left: twipsToPx(s.marginLeft),
  };
  const contentW = width - margin.left - margin.right;
  const bottom = height - margin.bottom;
  const markers = listMarkers(doc);
  const pages: Page[] = [{ index: 0, lines: [], boxes: [] }];
  let page = pages[0]!;
  let y = margin.top;
  const newPage = () => {
    page = { index: pages.length, lines: [], boxes: [] };
    pages.push(page);
    y = margin.top;
  };

  // Pre-break all paragraphs (cached by paragraph identity + geometry).
  const broken = doc.blocks.map((b) => {
    if (!isParagraph(b)) return null;
    const fmt = effectiveFormat(doc, b);
    const left = twipsToPx(fmt.indentLeft ?? 0);
    const right = twipsToPx(fmt.indentRight ?? 0);
    const first = twipsToPx(fmt.indentFirst ?? 0);
    const avail = contentW - left - right;
    const key = `${avail.toFixed(2)}|${first}|${b.style ?? ""}|${JSON.stringify(fmt)}`;
    let c = cache.get(b);
    if (!c || c.key !== key) {
      c = { key, lines: breakParagraph(doc, b, m, avail, Math.max(first, -left)) };
      cache.set(b, c);
    }
    return { fmt, left, right, first, avail, lines: cloneLines(c.lines) };
  });

  doc.blocks.forEach((b, bi) => {
    const info = broken[bi];
    if (!info) {
      if (y + OPAQUE_BLOCK_PX > bottom && y > margin.top) newPage();
      page.boxes.push({ block: bi, y, height: OPAQUE_BLOCK_PX - 8, left: margin.left, right: margin.left + contentW, label: (b as { label: string }).label });
      y += OPAQUE_BLOCK_PX;
      return;
    }
    const { fmt, left, first, avail, lines } = info;
    const before = twipsToPx(fmt.spaceBefore ?? 0);
    const after = twipsToPx(fmt.spaceAfter ?? 0);
    const paraH = lines.reduce((h, l) => h + l.height, 0);
    if (fmt.pageBreakBefore && y > margin.top) newPage();
    // keep-with-next: move if this paragraph plus the next one's first line won't fit.
    const nextInfo = broken[bi + 1];
    if (fmt.keepNext && nextInfo) {
      const need = before + paraH + after + twipsToPx(nextInfo.fmt.spaceBefore ?? 0) + (nextInfo.lines[0]?.height ?? 0);
      if (y + need > bottom && need <= bottom - margin.top && y > margin.top) newPage();
    }
    // orphan control: don't leave a lone first line at the page bottom.
    const top = y > margin.top ? before : 0;
    if (lines.length >= 2 && y + top + lines[0]!.height <= bottom && y + top + lines[0]!.height + lines[1]!.height > bottom && y > margin.top) newPage();
    y += y > margin.top ? before : 0;

    lines.forEach((l, li) => {
      if (y + l.height > bottom && y > margin.top) newPage();
      const indent = li === 0 ? Math.max(first, -left) : 0;
      alignLine(l, avail, fmt.align, li === lines.length - 1, li === 0 ? indent : 0);
      const lineLeft = margin.left + left;
      for (const f of l.frags) f.x += lineLeft;
      if (li === 0 && markers.has(bi)) {
        const st = l.frags.find((f) => f.kind === "text")?.style ?? runStyle(effectiveMarks(doc, b as Paragraph, {}));
        const text = markers.get(bi)!;
        const mw = m.width(text, st.font);
        const mx = lineLeft + Math.max(first, -left) - Math.max(0, -first);
        l.frags.unshift({ kind: "marker", x: Math.max(margin.left, mx), width: mw, text, style: st, start: 0, end: 0 });
      }
      page.lines.push({
        block: bi, y, height: l.height, baseline: l.baseline, left: lineLeft, right: lineLeft + avail,
        frags: l.frags, start: l.start, end: l.end, last: li === lines.length - 1,
      });
      y += l.height;
      if (l.pageBreakAfter) newPage();
    });
    y += after;
  });
  return { pages, width, height, margin };
}

/** Deterministic measurer for tests: each char = 0.5 × font px size. */
export const monoMeasurer: Measurer = {
  width(text: string, font: string) {
    const px = Number(/([\d.]+)px/.exec(font)?.[1] ?? 16);
    return Array.from(text).length * px * 0.5;
  },
};

/** Canvas-backed measurer with a per-font cache. */
export function canvasMeasurer(): Measurer {
  const ctx = document.createElement("canvas").getContext("2d");
  const cache = new Map<string, number>();
  return {
    width(text, font) {
      if (!ctx) return monoMeasurer.width(text, font);
      const key = `${font}\u0000${text}`;
      let w = cache.get(key);
      if (w === undefined) {
        ctx.font = font;
        w = ctx.measureText(text).width;
        if (cache.size > 50_000) cache.clear();
        cache.set(key, w);
      }
      return w;
    },
  };
}

export type { Block };
