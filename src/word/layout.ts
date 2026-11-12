/**
 * Document layout & pagination (P2-04, P2-06, P2-11).
 *
 * Coordinates are CSS pixels; OOXML twips convert at 15 twips/px (96 dpi),
 * EMU at 9525/px. Paragraphs break into lines from UAX #14–style clusters
 * (spaces, hyphens, CJK, soft/auto hyphenation points); lines are aligned,
 * justified and bidi-reordered; tab stops honour alignment and leaders.
 * The paginator flows paragraphs, tables (atomic rows, repeated header rows)
 * and opaque boxes through columns and pages, reserving room for footnotes,
 * and draws headers/footers with PAGE/NUMPAGES fields.
 */
import { hyphenateSync as hyphenateEn } from "hyphen/en-us";
import { hyphenateSync as hyphenateTr } from "hyphen/tr";
import {
  EMU_PER_PX, isParagraph, isTable, type Align, type Block, type Marks, type NoteKind, type NumberingDef,
  type ParaFormat, type Paragraph, type StyleInfo, type Table, type TabStop, type WordDocument,
} from "./model";
import { inlineLength } from "./ops";

export interface Measurer {
  width(text: string, font: string): number;
}

export const twipsToPx = (t: number) => t / 15;
const halfPtToPx = (hp: number) => (hp / 2) * (96 / 72);
const SHY = /­/g;

export type ContainerRef =
  | { type: "cell"; table: number; row: number; col: number }
  | { type: "note"; kind: NoteKind; id: number }
  | { type: "header"; index: number };

export function sameContainer(a: ContainerRef | null | undefined, b: ContainerRef | null | undefined): boolean {
  if (!a || !b) return !a && !b;
  return JSON.stringify(a) === JSON.stringify(b);
}

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
  revision: "insert" | "delete" | null;
  rtl: boolean;
}

export interface Frag {
  kind: "text" | "tab" | "opaque" | "marker" | "image" | "shape" | "note" | "hyphen";
  x: number;
  width: number;
  text: string;
  style: RunStyle;
  /** Paragraph offsets covered [start, end). Markers cover none. */
  start: number;
  end: number;
  /** Object height (images/shapes). */
  height?: number;
  relId?: string;
  preset?: string;
  stroke?: string;
  fill?: string | null;
  leader?: string | null;
  rtl?: boolean;
}

export interface Line {
  block: number;
  container: ContainerRef | null;
  /** Page-relative top. */
  y: number;
  height: number;
  baseline: number;
  left: number;
  right: number;
  frags: Frag[];
  start: number;
  end: number;
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

export interface Deco {
  kind: "rect" | "line";
  x: number;
  y: number;
  w: number;
  h: number;
  fill?: string;
  stroke?: string;
}

export interface HitArea {
  container: ContainerRef;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Page {
  index: number;
  lines: Line[];
  boxes: BlockBox[];
  decos: Deco[];
  areas: HitArea[];
}

export interface Layout {
  pages: Page[];
  width: number;
  height: number;
  margin: { top: number; right: number; bottom: number; left: number };
}

export interface LayoutOptions {
  /** Show tracked changes (default true); false = final view. */
  showMarkup?: boolean;
}

const TAB_PX = 48;
const OPAQUE_BLOCK_PX = 44;
const CELL_PAD = 108 / 15;
const HIGHLIGHTS: Record<string, string> = {
  yellow: "#fff59d", green: "#a5d6a7", cyan: "#80deea", magenta: "#f48fb1", blue: "#90caf9", red: "#ef9a9a",
  darkBlue: "#5c6bc0", darkCyan: "#4db6ac", darkGreen: "#66bb6a", darkMagenta: "#ab47bc", darkRed: "#e57373",
  darkYellow: "#d4c15a", darkGray: "#9e9e9e", lightGray: "#e0e0e0", black: "#424242",
};
const AUTHOR_COLORS = ["#b45309", "#7c3aed", "#0f766e", "#be123c", "#1d4ed8", "#4d7c0f"];

export function authorColor(author: string): string {
  let h = 0;
  for (const c of author) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return AUTHOR_COLORS[h % AUTHOR_COLORS.length]!;
}

export function resolveStyle(doc: WordDocument, id: string | undefined): StyleInfo | undefined {
  if (id) {
    const s = doc.styles.find((x) => x.id === id);
    if (s) return s;
  }
  return doc.styles.find((s) => s.kind === "paragraph" && s.isDefault);
}

function stripUndef<T extends object>(o: T): Partial<T> {
  return Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined)) as Partial<T>;
}

export function effectiveFormat(doc: WordDocument, p: Paragraph): ParaFormat {
  const style = resolveStyle(doc, p.style);
  const fmt: ParaFormat = { ...doc.defaultPara, ...stripUndef(style?.para ?? {}) };
  const direct: ParaFormat = {
    align: p.align, spaceBefore: p.spaceBefore, spaceAfter: p.spaceAfter, line: p.line, indentLeft: p.indentLeft,
    indentRight: p.indentRight, indentFirst: p.indentFirst, keepNext: p.keepNext, pageBreakBefore: p.pageBreakBefore,
    bidi: p.bidi || undefined, tabs: p.tabs?.length ? p.tabs : undefined,
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

export function effectiveMarks(doc: WordDocument, p: Paragraph, m: Marks): Marks {
  const style = resolveStyle(doc, p.style);
  const charStyle = m.style ? doc.styles.find((s) => s.id === m.style)?.run : undefined;
  return { ...doc.defaultRun, ...stripUndef(style?.run ?? {}), ...stripUndef(charStyle ?? {}), ...stripUndef(m) };
}

/** CSS font stack with metric-compatible open fallbacks for Office fonts. */
export function fontStack(family: string): string {
  const fallbacks: Record<string, string> = {
    calibri: "Carlito", cambria: "Caladea", arial: '"Liberation Sans", Arimo', helvetica: '"Liberation Sans", Arimo',
    "times new roman": '"Liberation Serif", Tinos', "courier new": '"Liberation Mono", Cousine', georgia: "Gelasio",
    "segoe ui": '"Selawik", "Open Sans"', consolas: '"Inconsolata"',
  };
  const generic = /mono|consolas|courier/i.test(family) ? "ui-monospace, monospace" : /times|georgia|cambria|serif|garamond/i.test(family) ? "serif" : "system-ui, sans-serif";
  const fb = fallbacks[family.toLowerCase()];
  return `"${family}", ${fb ? fb + ", " : ""}${generic}`;
}

export function runStyle(m: Marks): RunStyle {
  let sizePx = halfPtToPx(m.size ?? 22);
  const vert = m.vertAlign ?? null;
  if (vert) sizePx *= 0.65;
  const family = m.font ?? "Calibri";
  const font = `${m.italic ? "italic " : ""}${m.bold ? "700 " : "400 "}${sizePx.toFixed(2)}px ${fontStack(family)}`;
  const rev = m.revision?.kind ?? null;
  const revColor = m.revision ? authorColor(m.revision.author) : null;
  return {
    font,
    family,
    sizePx,
    bold: !!m.bold,
    italic: !!m.italic,
    underline: !!m.underline || !!m.link || rev === "insert",
    strike: !!m.strike || rev === "delete",
    color: revColor ?? (m.link && !m.color ? "#1d4ed8" : `#${m.color ?? "1f2328"}`),
    highlight: m.highlight ? (HIGHLIGHTS[m.highlight] ?? null) : null,
    vert,
    link: m.link ?? null,
    revision: rev,
    rtl: !!m.rtl,
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

/** Visit every paragraph in reading order (body, then table cells inline). */
function eachParagraph(blocks: Block[], fn: (p: Paragraph, topIndex: number) => void, top = -1) {
  blocks.forEach((b, i) => {
    const idx = top >= 0 ? top : i;
    if (isParagraph(b)) fn(b, idx);
    else if (isTable(b)) for (const r of b.rows) for (const c of r.cells) eachParagraph(c.blocks, fn, idx);
  });
}

function computeMarkers(doc: WordDocument): Map<Paragraph, string> {
  const out = new Map<Paragraph, string>();
  const counters = new Map<number, number[]>();
  eachParagraph(doc.blocks, (b) => {
    if (!b.list) return;
    const { numId, level } = b.list;
    const def = doc.numbering.find((d) => d.numId === numId);
    const lvl = def?.levels[level];
    if (!def || !lvl) return;
    const c = counters.get(numId) ?? [];
    c[level] = (c[level] ?? (lvl.start - 1)) + 1;
    c.length = level + 1;
    counters.set(numId, c);
    if (lvl.format === "bullet") {
      out.set(b, lvl.text.replace("", "•") || "•");
      return;
    }
    out.set(b, lvl.text.replace(/%(\d)/g, (_, d: string) => {
      const L = Number(d) - 1;
      const ld = def.levels[L];
      return formatNumber(c[L] ?? ld?.start ?? 1, ld?.format ?? "decimal");
    }));
  });
  return out;
}

/** Marker text for every top-level list paragraph (by block index). */
export function listMarkers(doc: WordDocument): Map<number, string> {
  const byPara = computeMarkers(doc);
  const out = new Map<number, string>();
  doc.blocks.forEach((b, i) => {
    if (isParagraph(b) && byPara.has(b)) out.set(i, byPara.get(b)!);
  });
  return out;
}

/** Footnote numbers (1, 2, …) and endnote numbers (i, ii, …) in order of reference. */
export function noteNumbers(doc: WordDocument): Map<string, string> {
  const out = new Map<string, string>();
  let fn = 0;
  let en = 0;
  eachParagraph(doc.blocks, (p) => {
    for (const i of p.inlines) {
      if (i.type !== "note") continue;
      const key = `${i.kind}:${i.id}`;
      if (out.has(key)) continue;
      out.set(key, i.kind === "footnote" ? String(++fn) : toRoman(++en));
    }
  });
  return out;
}

// ---- line breaking (UAX #14 subset) ----------------------------------------------

const CJK = /[⺀-鿿가-힯豈-﫿぀-ヿ＀-￯]/;
const CLOSE = /[)\]}»›”’.,;:!?、。，．！？」』）〕】%‰]/;
const OPEN = /[([{«‹“‘「『（〔【¿¡$£€]/;
const GLUE = /[   ‑⁠﻿]/;
const SPACE = /[ \t　 -  -  ]/;

/** Break opportunity *after* index i (between chars[i] and chars[i+1]). */
function canBreakAfter(chars: string[], i: number, extra?: Set<number>): boolean {
  const a = chars[i]!;
  const b = chars[i + 1];
  if (b === undefined) return false;
  if (GLUE.test(a) || GLUE.test(b) || b === "⁠") return false;
  if (a === "​") return true;
  if (CLOSE.test(b) || SPACE.test(b)) return false;
  if (OPEN.test(a)) return false;
  if (SPACE.test(a)) return true;
  if (a === "­") return true;
  if ((a === "-" || a === "‐") && !/\d/.test(b) && i > 0 && !SPACE.test(chars[i - 1]!)) return true;
  if (a === "–" || a === "—" || b === "—") return true;
  if (a === "/" && i > 0 && !/\d/.test(b)) return true;
  if (CJK.test(a) || (CJK.test(b) && !CLOSE.test(b))) return true;
  return extra?.has(i) ?? false;
}

/** Split text into unbreakable clusters (trailing spaces stay attached). */
export function tokenize(text: string, extra?: Set<number>): string[] {
  const chars = Array.from(text);
  const out: string[] = [];
  let cur = "";
  chars.forEach((c, i) => {
    cur += c;
    if (canBreakAfter(chars, i, extra)) {
      out.push(cur);
      cur = "";
    }
  });
  if (cur) out.push(cur);
  return out;
}

let hyphenCache = new Map<string, Set<number>>();
/** Auto-hyphenation points (code-point indices after which a break may occur). */
export function hyphenationPoints(text: string, lang: "en" | "tr" = "en"): Set<number> {
  const key = `${lang}\u0000${text}`;
  const hit = hyphenCache.get(key);
  if (hit) return hit;
  const pts = new Set<number>();
  const chars = Array.from(text);
  const re = /\p{L}{6,}/gu;
  let m: RegExpExecArray | null;
  const prefixLen = (s: string) => Array.from(s).length;
  while ((m = re.exec(text))) {
    const startCp = prefixLen(text.slice(0, m.index));
    const h = (lang === "tr" ? hyphenateTr : hyphenateEn)(m[0], { hyphenChar: "­" });
    let cp = startCp - 1;
    for (const c of h) {
      if (c === "­") pts.add(cp);
      else cp++;
    }
  }
  void chars;
  if (hyphenCache.size > 5000) hyphenCache = new Map();
  hyphenCache.set(key, pts);
  return pts;
}

// ---- bidi ----------------------------------------------------------------------

const RTL_CHARS = /[֐-ࣿיִ-﷿ﹰ-﻿]/;
const LTR_CHARS = /[A-Za-zÀ-ɏͰ-ϿЀ-ӿḀ-ỿ]|[⺀-鿿]/;

function fragLevel(f: Frag, base: number): number {
  if (f.kind === "marker") return base;
  if (f.style.rtl || RTL_CHARS.test(f.text)) return 1;
  if (LTR_CHARS.test(f.text) || /\d/.test(f.text)) return base === 1 ? 2 : 0;
  return base;
}

/** Reorder a line's fragments into visual order (UAX #9 rule L2 on fragments). */
export function reorderBidi(frags: Frag[], rtlBase: boolean): Frag[] {
  const base = rtlBase ? 1 : 0;
  const levels = frags.map((f) => fragLevel(f, base));
  if (levels.every((l) => l === 0)) return frags;
  const start = frags.length ? Math.min(...frags.map((f) => f.x)) : 0;
  let order = frags.map((f, i) => ({ f, l: levels[i]! }));
  const max = Math.max(...levels);
  for (let lvl = max; lvl >= 1; lvl--) {
    const next: typeof order = [];
    let i = 0;
    while (i < order.length) {
      if (order[i]!.l >= lvl) {
        let j = i;
        while (j < order.length && order[j]!.l >= lvl) j++;
        next.push(...order.slice(i, j).reverse());
        i = j;
      } else next.push(order[i++]!);
    }
    order = next;
  }
  let x = start;
  return order.map(({ f, l }) => {
    const nf = { ...f, x, rtl: l % 2 === 1 };
    x += f.width;
    return nf;
  });
}

// ---- paragraph items ------------------------------------------------------------

interface Item {
  kind: "text" | "tab" | "opaque" | "break" | "image" | "shape" | "note" | "marker";
  text: string;
  style: RunStyle;
  start: number;
  end: number;
  width: number;
  trailing: number;
  breakAfter: boolean;
  hyphen?: boolean;
  breakKind?: "line" | "page" | "column";
  height?: number;
  relId?: string;
  preset?: string;
  stroke?: string;
  fill?: string | null;
}

export interface ParaContext {
  showMarkup: boolean;
  noteNumbers: Map<string, string>;
  /** Comment ids open at paragraph start (comments can span paragraphs). */
  openComments: Set<string>;
  hyphenate: false | "en" | "tr";
  /** Max object width (images scale down to fit). */
  maxObjectWidth: number;
  prefix?: string;
}

const defaultParaCtx = (): ParaContext => ({ showMarkup: true, noteNumbers: new Map(), openComments: new Set(), hyphenate: false, maxObjectWidth: 10_000 });

function mw(m: Measurer, text: string, font: string): number {
  return m.width(text.replace(SHY, ""), font);
}

function buildItems(doc: WordDocument, p: Paragraph, m: Measurer, ctx: ParaContext): Item[] {
  const items: Item[] = [];
  let offset = 0;
  const open = ctx.openComments;
  const base = runStyle(effectiveMarks(doc, p, {}));
  if (ctx.prefix) {
    const st = { ...base, vert: "superscript" as const, sizePx: base.sizePx * 0.65, font: base.font.replace(/[\d.]+px/, `${(base.sizePx * 0.65).toFixed(2)}px`) };
    items.push({ kind: "marker", text: ctx.prefix, style: st, start: 0, end: 0, width: mw(m, ctx.prefix, st.font), trailing: 0, breakAfter: false });
  }
  for (const inl of p.inlines) {
    const len = inlineLength(inl);
    if (inl.type === "text") {
      let style = runStyle(effectiveMarks(doc, p, inl.marks));
      if (open.size && !style.highlight) style = { ...style, highlight: "#fde68a" };
      if (!ctx.showMarkup && inl.marks.revision?.kind === "delete") {
        items.push({ kind: "opaque", text: "", style, start: offset, end: offset + len, width: 0, trailing: 0, breakAfter: false });
        offset += len;
        continue;
      }
      if (!ctx.showMarkup && inl.marks.revision) style = runStyle(effectiveMarks(doc, p, { ...inl.marks, revision: undefined }));
      const extra = ctx.hyphenate ? hyphenationPoints(inl.text, ctx.hyphenate) : undefined;
      let o = offset;
      for (const tok of tokenize(inl.text, extra)) {
        const trailingText = tok.match(/\s+$/)?.[0] ?? "";
        const last = Array.from(tok).pop() ?? "";
        const cpEnd = o + tok.length;
        const autoHyphen = !!extra && !/[\s\-­]/.test(last) && /\p{L}/u.test(last) && cpEnd < offset + inl.text.length && /\p{L}/u.test(inl.text[cpEnd - offset] ?? "");
        items.push({
          kind: "text",
          text: tok,
          style,
          start: o,
          end: cpEnd,
          width: mw(m, tok, style.font),
          trailing: trailingText ? mw(m, trailingText, style.font) : 0,
          breakAfter: /[\s\-‐–—/­​]/.test(last) || CJK.test(last) || autoHyphen,
          hyphen: last === "­" || autoHyphen,
        });
        o = cpEnd;
      }
    } else if (inl.type === "tab") {
      const style = runStyle(effectiveMarks(doc, p, inl.marks));
      items.push({ kind: "tab", text: "\t", style, start: offset, end: offset + 1, width: 0, trailing: 0, breakAfter: true });
    } else if (inl.type === "break") {
      items.push({ kind: "break", text: "", style: base, start: offset, end: offset + 1, width: 0, trailing: 0, breakAfter: true, breakKind: inl.kind });
    } else if (inl.type === "image" || inl.type === "shape") {
      let w = inl.cx / EMU_PER_PX;
      let h = inl.cy / EMU_PER_PX;
      if (w > ctx.maxObjectWidth) {
        h = (h * ctx.maxObjectWidth) / w;
        w = ctx.maxObjectWidth;
      }
      items.push({
        kind: inl.type, text: "", style: base, start: offset, end: offset + 1, width: w, height: h, trailing: 0, breakAfter: true,
        relId: inl.type === "image" ? inl.relId : undefined,
        preset: inl.type === "shape" ? inl.preset : undefined,
        stroke: inl.type === "shape" ? inl.stroke : undefined,
        fill: inl.type === "shape" ? (inl.fill ?? null) : undefined,
      });
    } else if (inl.type === "note") {
      const num = ctx.noteNumbers.get(`${inl.kind}:${inl.id}`) ?? "?";
      const st = runStyle({ ...effectiveMarks(doc, p, {}), vertAlign: "superscript" });
      items.push({ kind: "note", text: num, style: st, start: offset, end: offset + 1, width: mw(m, num, st.font), trailing: 0, breakAfter: false });
    } else if (inl.type === "comment") {
      if (inl.edge === "start") open.add(inl.id);
      if (inl.edge === "end") open.delete(inl.id);
      items.push({ kind: "opaque", text: "", style: base, start: offset, end: offset + 1, width: 0, trailing: 0, breakAfter: false });
    } else {
      const label = inl.label ? `⟦${inl.label}⟧` : "";
      const w = label ? mw(m, label, base.font) + 6 : 0;
      items.push({ kind: "opaque", text: label, style: base, start: offset, end: offset + 1, width: w, trailing: 0, breakAfter: !!label });
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
  const textFrags = frags.filter((f) => f.kind !== "image" && f.kind !== "shape");
  const sizes = textFrags.length ? textFrags.map((f) => (f.style.vert ? f.style.sizePx / 0.65 : f.style.sizePx)) : [fallback.sizePx];
  const max = Math.max(...sizes);
  const natural = max * 1.22;
  const objH = Math.max(0, ...frags.filter((f) => f.height).map((f) => f.height!));
  const asc = max * 0.98;
  const textH = natural * factor;
  if (objH > asc) {
    const height = objH + (natural - asc);
    return { height: Math.max(height, textH), baseline: objH };
  }
  return { height: textH, baseline: (textH - natural) / 2 + asc };
}

/** Width of items following index i up to the next tab/break (for right/center tabs). */
function segmentWidth(items: Item[], i: number, decimal: boolean): number {
  let w = 0;
  for (let j = i + 1; j < items.length; j++) {
    const it = items[j]!;
    if (it.kind === "tab" || it.kind === "break") break;
    if (decimal && it.kind === "text") {
      const k = it.text.search(/[.,]/);
      if (k >= 0) return w + it.width * (k / Math.max(1, it.text.length));
    }
    w += it.width;
  }
  return w;
}

interface TabCtx {
  /** Indent origin in content coordinates (px). */
  origin: number;
  stops: TabStop[];
}

function nextTab(x: number, tc: TabCtx): { pos: number; stop: TabStop | null } {
  const cx = x + tc.origin;
  for (const s of tc.stops) {
    if (s.align === "clear" || s.align === "bar") continue;
    const sx = twipsToPx(s.pos);
    if (sx > cx + 0.5) return { pos: sx - tc.origin, stop: s };
  }
  return { pos: (Math.floor(cx / TAB_PX) + 1) * TAB_PX - tc.origin, stop: null };
}

/** Break one paragraph into lines for a given available width. */
export function breakParagraph(
  doc: WordDocument,
  p: Paragraph,
  m: Measurer,
  avail: number,
  firstIndent: number,
  ctx: ParaContext = defaultParaCtx(),
  tabs: TabCtx = { origin: 0, stops: [] },
): RawLine[] {
  const fmt = effectiveFormat(doc, p);
  const factor = Math.max(0.5, (fmt.line ?? 240) / 240);
  const items = buildItems(doc, p, m, { ...ctx, maxObjectWidth: Math.min(ctx.maxObjectWidth, avail) });
  const fallback = runStyle(effectiveMarks(doc, p, {}));
  const lines: RawLine[] = [];
  let frags: Frag[] = [];
  let x = firstIndent;
  let lineStart = 0;
  const hyphenW = mw(m, "-", fallback.font);

  const toFrag = (c: Item, xx: number, text = c.text, start = c.start, end = c.end, width = c.width): Frag => ({
    kind: c.kind === "break" ? "text" : (c.kind as Frag["kind"]), x: xx, width, text, style: c.style, start, end,
    height: c.height, relId: c.relId, preset: c.preset, stroke: c.stroke, fill: c.fill,
  });

  const finish = (end: number, forced: boolean, pageBreakAfter = false) => {
    let last = frags[frags.length - 1];
    if (!forced && last?.kind === "text" && (last.text.endsWith("­") || (last as Frag & { autoHyphen?: boolean }).autoHyphen)) {
      frags.push({ kind: "hyphen", x: last.x + last.width, width: hyphenW, text: "-", style: last.style, start: last.end, end: last.end });
      last = frags[frags.length - 1];
    }
    const trailing = last?.kind === "text" ? mw(m, last.text.match(/\s+$/)?.[0] ?? "", last.style.font) : 0;
    const width = (last ? last.x + last.width - trailing : x) - (lines.length === 0 ? firstIndent : 0);
    const met = lineMetrics(frags, fallback, factor);
    lines.push({ frags, start: lineStart, end, height: met.height, baseline: met.baseline, width, forced, pageBreakAfter });
    frags = [];
    x = 0;
    lineStart = end;
  };

  let i = 0;
  while (i < items.length) {
    const it = items[i]!;
    if (it.kind === "break") {
      frags.push(toFrag(it, x, "", it.start, it.end, 0));
      finish(it.end, true, it.breakKind === "page" || it.breakKind === "column");
      i++;
      continue;
    }
    if (it.kind === "tab") {
      const t = nextTab(x, tabs);
      let target = t.pos;
      if (t.stop && (t.stop.align === "right" || t.stop.align === "center" || t.stop.align === "decimal")) {
        const seg = segmentWidth(items, i, t.stop.align === "decimal");
        target = t.stop.align === "center" ? t.pos - seg / 2 : t.pos - seg;
      }
      let w = Math.max(t.stop ? 0 : 4, target - x);
      if (x + w > avail + 0.01 && frags.length) {
        finish(it.start, false);
        w = Math.max(4, nextTab(0, tabs).pos);
      }
      frags.push({ ...toFrag(it, x, "", it.start, it.end, w), leader: t.stop?.leader ?? null });
      x += w;
      i++;
      continue;
    }
    // Gather an unbreakable cluster.
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
    const lastItem = cluster[cluster.length - 1]!;
    const trailing = lastItem.trailing;
    const need = clusterW - trailing + (lastItem.hyphen ? hyphenW : 0);
    if (x + need > avail + 0.01 && frags.length > 0 && frags.some((f) => f.kind !== "marker")) finish(cluster[0]!.start, false);
    if (x + clusterW - trailing > avail + 0.01 && (frags.length === 0 || frags.every((f) => f.kind === "marker"))) {
      for (const c of cluster) {
        if (c.kind !== "text") {
          frags.push(toFrag(c, x));
          x += c.width;
          continue;
        }
        let rest = c.text;
        let start = c.start;
        while (rest) {
          let n = rest.length;
          while (n > 1 && x + mw(m, rest.slice(0, n), c.style.font) > avail) n--;
          const piece = rest.slice(0, n);
          const w = mw(m, piece, c.style.font);
          if (x + w > avail + 0.01 && frags.length) {
            finish(start, false);
            continue;
          }
          frags.push(toFrag(c, x, piece, start, start + piece.length, w));
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
      const f = toFrag(c, x);
      if (c.hyphen && c === lastItem && !c.text.endsWith("­")) (f as Frag & { autoHyphen?: boolean }).autoHyphen = true;
      frags.push(f);
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
    const content = line.frags.filter((f) => f.kind !== "hyphen");
    const gaps = content.slice(0, -1).filter((f) => f.kind === "text" && /\s$/.test(f.text)).length;
    if (!gaps) return;
    const per = slack / gaps;
    let shift = 0;
    const lastContent = content[content.length - 1];
    for (const f of line.frags) {
      f.x += shift;
      if (f.kind === "text" && /\s$/.test(f.text) && f !== lastContent) shift += per;
    }
  }
}

// ---- flows ------------------------------------------------------------------------

interface FlowLine extends RawLine {
  block: number;
  container: ContainerRef | null;
  last: boolean;
  /** Relative to the flow's content left. */
  left: number;
  right: number;
}

interface CellLayout {
  x: number;
  w: number;
  lines: FlowLine[];
  boxes: { block: number; y: number; height: number; label: string }[];
  height: number;
  shading?: string;
  vMerge?: "restart" | "continue";
  container: ContainerRef;
}

interface RowLayout {
  height: number;
  header: boolean;
  cells: CellLayout[];
}

type FlowItem =
  | { kind: "para"; block: number; container: ContainerRef | null; lines: FlowLine[]; before: number; after: number; keepNext: boolean; pageBreakBefore: boolean }
  | { kind: "table"; block: number; rows: RowLayout[]; width: number; borders: boolean }
  | { kind: "box"; block: number; label: string; height: number };

interface Env {
  doc: WordDocument;
  m: Measurer;
  markers: Map<Paragraph, string>;
  ctx: ParaContext;
}

interface Cached {
  key: string;
  lines: RawLine[];
}
const cache = new WeakMap<Paragraph, Cached>();

function cloneLines(lines: RawLine[]): RawLine[] {
  return lines.map((l) => ({ ...l, frags: l.frags.map((f) => ({ ...f })) }));
}

function layoutParagraph(env: Env, p: Paragraph, width: number, block: number, container: ContainerRef | null, prefix?: string): FlowItem {
  const { doc, m } = env;
  const fmt = effectiveFormat(doc, p);
  const left = twipsToPx(fmt.indentLeft ?? 0);
  const right = twipsToPx(fmt.indentRight ?? 0);
  const first = twipsToPx(fmt.indentFirst ?? 0);
  const avail = Math.max(20, width - left - right);
  const firstIndent = Math.max(first, -left);
  const ctx: ParaContext = { ...env.ctx, prefix, openComments: env.ctx.openComments };
  const commentsKey = [...ctx.openComments].join(",");
  const hasComments = commentsKey !== "" || p.inlines.some((i) => i.type === "comment");
  const hasNotes = p.inlines.some((i) => i.type === "note");
  const key = `${avail.toFixed(2)}|${firstIndent}|${JSON.stringify(fmt)}|${ctx.showMarkup}|${ctx.hyphenate}|${prefix ?? ""}`;
  let raw: RawLine[];
  const c = cache.get(p);
  if (c && c.key === key && !hasComments && !hasNotes) {
    raw = cloneLines(c.lines);
  } else {
    raw = breakParagraph(doc, p, m, avail, firstIndent, ctx, { origin: left, stops: fmt.tabs ?? [] });
    if (!hasComments && !hasNotes) cache.set(p, { key, lines: cloneLines(raw) });
  }
  const rtl = !!fmt.bidi;
  const align: Align | undefined = rtl ? (fmt.align === "right" ? "left" : fmt.align === "center" || fmt.align === "justify" ? fmt.align : "right") : fmt.align;
  const marker = env.markers.get(p);
  const lines: FlowLine[] = raw.map((l, li) => {
    if (rtl || l.frags.some((f) => f.style.rtl || RTL_CHARS.test(f.text))) l.frags = reorderBidi(l.frags, rtl);
    alignLine(l, avail, align, li === raw.length - 1, li === 0 ? firstIndent : 0);
    for (const f of l.frags) f.x += left;
    if (li === 0 && marker) {
      const st = l.frags.find((f) => f.kind === "text")?.style ?? runStyle(effectiveMarks(doc, p, {}));
      const mwid = mw(m, marker, st.font);
      const mx = left + firstIndent - Math.max(0, -first);
      l.frags.unshift({ kind: "marker", x: Math.max(0, mx), width: mwid, text: marker, style: st, start: 0, end: 0 });
    }
    return { ...l, block, container, last: li === raw.length - 1, left, right: left + avail };
  });
  return {
    kind: "para", block, container, lines,
    before: twipsToPx(fmt.spaceBefore ?? 0), after: twipsToPx(fmt.spaceAfter ?? 0),
    keepNext: !!fmt.keepNext, pageBreakBefore: !!fmt.pageBreakBefore,
  };
}

/** Lay out blocks inside a container (cell/note/header) as a single unpaginated column. */
function layoutStatic(env: Env, blocks: Block[], width: number, container: (i: number) => ContainerRef | null, prefix?: string) {
  const lines: FlowLine[] = [];
  const boxes: { block: number; y: number; height: number; label: string }[] = [];
  let y = 0;
  blocks.forEach((b, i) => {
    if (!isParagraph(b)) {
      boxes.push({ block: i, y, height: OPAQUE_BLOCK_PX - 8, label: isTable(b) ? "Table" : b.label });
      y += OPAQUE_BLOCK_PX;
      return;
    }
    const it = layoutParagraph(env, b, width, i, container(i), i === 0 ? prefix : undefined) as Extract<FlowItem, { kind: "para" }>;
    y += i > 0 ? it.before : 0;
    for (const l of it.lines) {
      lines.push({ ...l, baseline: l.baseline, height: l.height, frags: l.frags, ...({ y } as object) } as FlowLine & { y: number });
      y += l.height;
    }
    y += i < blocks.length - 1 ? it.after : 0;
  });
  return { lines: lines as (FlowLine & { y: number })[], boxes, height: y };
}

function tableColumns(t: Table, width: number): number[] {
  const cols = t.grid.length || Math.max(1, ...t.rows.map((r) => r.cells.reduce((n, c) => n + (c.span || 1), 0)));
  let ws = t.grid.length ? t.grid.map(twipsToPx) : Array.from({ length: cols }, () => width / cols);
  const total = ws.reduce((a, b) => a + b, 0);
  if (total > width || total <= 0) ws = ws.map((w) => (total > 0 ? (w * width) / total : width / cols));
  return ws;
}

function layoutTable(env: Env, t: Table, block: number, width: number): FlowItem {
  const ws = tableColumns(t, width);
  const xs = ws.reduce<number[]>((acc, w, i) => [...acc, (acc[i] ?? 0) + w], [0]);
  const rows: RowLayout[] = t.rows.map((r, ri) => {
    let g = 0;
    const cells: CellLayout[] = r.cells.map((c, ci) => {
      const span = Math.max(1, c.span || 1);
      const x = xs[Math.min(g, xs.length - 1)] ?? 0;
      const w = (xs[Math.min(g + span, xs.length - 1)] ?? width) - x;
      g += span;
      const container: ContainerRef = { type: "cell", table: block, row: ri, col: ci };
      const inner = Math.max(10, w - CELL_PAD * 2);
      const st = c.vMerge === "continue" ? { lines: [], boxes: [], height: 0 } : layoutStatic(env, c.blocks, inner, () => container);
      return { x, w, lines: st.lines, boxes: st.boxes, height: st.height + 2, shading: c.shading, vMerge: c.vMerge, container };
    });
    const height = Math.max(20, ...cells.map((c) => c.height));
    return { height, header: !!r.header, cells };
  });
  return { kind: "table", block, rows, width: xs[xs.length - 1] ?? width, borders: t.borders };
}

function layoutFlow(env: Env, blocks: Block[], width: number): FlowItem[] {
  return blocks.map((b, i) => {
    if (isParagraph(b)) return layoutParagraph(env, b, width, i, null);
    if (isTable(b)) return layoutTable(env, b, i, width);
    return { kind: "box", block: i, label: b.label, height: OPAQUE_BLOCK_PX };
  });
}

/** Replace PAGE / NUMPAGES field results inside header/footer blocks. */
export function substituteFields(blocks: Block[], page: number, total: number): Block[] {
  return blocks.map((b) => {
    if (!isParagraph(b)) return b;
    let state: "none" | "instr" | "result" = "none";
    let instr = "";
    let value: string | null = null;
    let replaced = false;
    const inlines = b.inlines.map((i) => {
      if (i.type === "opaque") {
        if (i.xml.includes('fldCharType="begin"')) {
          state = "instr";
          instr = "";
        } else if (i.xml.includes('fldCharType="separate"')) {
          const code = instr.trim().split(/\s+/)[0]?.toUpperCase();
          value = code === "PAGE" ? String(page) : code === "NUMPAGES" || code === "SECTIONPAGES" ? String(total) : null;
          state = "result";
          replaced = false;
        } else if (i.xml.includes('fldCharType="end"')) {
          state = "none";
          value = null;
        } else if (state === "instr" && i.xml.includes("instrText")) {
          instr += i.xml.replace(/<[^>]+>/g, "");
        }
        return i;
      }
      if (state === "result" && value !== null && i.type === "text") {
        if (replaced) return { ...i, text: "" };
        replaced = true;
        return { ...i, text: value };
      }
      return i;
    });
    return { ...b, inlines };
  });
}

// ---- pagination ---------------------------------------------------------------------

/** Lay out and paginate the whole document. */
export function layoutDocument(doc: WordDocument, m: Measurer, opts: LayoutOptions = {}): Layout {
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
  const nCols = Math.max(1, s.columns?.count ?? 1);
  const gap = nCols > 1 ? twipsToPx(s.columns?.space ?? 720) : 0;
  const colW = (contentW - gap * (nCols - 1)) / nCols;
  const lang = /[əƏğĞ]/.test(JSON.stringify(doc.blocks).slice(0, 20000)) ? "tr" : "en";
  const env: Env = {
    doc, m, markers: computeMarkers(doc),
    ctx: { showMarkup: opts.showMarkup ?? true, noteNumbers: noteNumbers(doc), openComments: new Set(), hyphenate: doc.settings?.autoHyphenation ? lang : false, maxObjectWidth: colW },
  };
  const flow = layoutFlow(env, doc.blocks, colW);

  // Footnote flows (cached per note).
  const noteFlows = new Map<string, ReturnType<typeof layoutStatic>>();
  const noteFlow = (kind: NoteKind, id: number) => {
    const key = `${kind}:${id}`;
    let f = noteFlows.get(key);
    if (!f) {
      const note = doc.notes?.find((n) => n.kind === kind && n.id === id);
      const blocks = note?.blocks.length ? note.blocks : [{ type: "paragraph" as const, inlines: [] }];
      const fenv: Env = { ...env, ctx: { ...env.ctx, openComments: new Set() } };
      f = layoutStatic(fenv, blocks, colW, () => ({ type: "note", kind, id }), (env.ctx.noteNumbers.get(key) ?? "?") + " ");
      noteFlows.set(key, f);
    }
    return f;
  };

  const pages: Page[] = [];
  let page!: Page;
  let col = 0;
  let y = margin.top;
  let pageNotes: { kind: NoteKind; id: number }[] = [];
  let notesH = 0;
  const NOTE_SEP = 14;
  const bottom = () => height - margin.bottom - (notesH ? notesH + NOTE_SEP : 0);
  const colX = () => margin.left + col * (colW + gap);

  const finishPage = () => {
    if (!page || !pageNotes.length) return;
    let ny = height - margin.bottom - notesH;
    page.decos.push({ kind: "line", x: margin.left, y: ny - NOTE_SEP / 2, w: Math.min(144, colW / 3), h: 0.75, stroke: "#9ca3af" });
    for (const n of pageNotes) {
      const f = noteFlow(n.kind, n.id);
      const container: ContainerRef = { type: "note", kind: n.kind, id: n.id };
      page.areas.push({ container, x: margin.left, y: ny, w: colW, h: f.height });
      for (const l of f.lines) page.lines.push(toLine(l, margin.left, ny + l.y));
      ny += f.height;
    }
  };
  const newPage = () => {
    finishPage();
    page = { index: pages.length, lines: [], boxes: [], decos: [], areas: [] };
    pages.push(page);
    col = 0;
    y = margin.top;
    pageNotes = [];
    notesH = 0;
  };
  const nextColumn = () => {
    if (col + 1 < nCols) {
      col++;
      y = margin.top;
    } else newPage();
  };
  const atTop = () => y <= margin.top + 0.01;
  const toLine = (l: FlowLine & { y?: number }, ox: number, ly: number): Line => ({
    block: l.block, container: l.container, y: ly, height: l.height, baseline: l.baseline,
    left: ox + l.left, right: ox + l.right, frags: l.frags.map((f) => ({ ...f, x: f.x + ox })),
    start: l.start, end: l.end, last: l.last,
  });
  const notesIn = (l: FlowLine) =>
    l.frags.filter((f) => f.kind === "note").map((f) => {
      const p = doc.blocks[l.block];
      const inl = p && isParagraph(p) ? findInlineAt(p, f.start) : null;
      return inl && inl.type === "note" ? { kind: inl.kind, id: inl.id } : null;
    }).filter((n): n is { kind: NoteKind; id: number } => !!n && n.kind === "footnote");

  newPage();
  const endnotes: { kind: NoteKind; id: number }[] = [];
  flow.forEach((item, idx) => {
    if (item.kind === "box") {
      if (y + item.height > bottom() && !atTop()) nextColumn();
      page.boxes.push({ block: item.block, y, height: item.height - 8, left: colX(), right: colX() + colW, label: item.label });
      y += item.height;
      return;
    }
    if (item.kind === "table") {
      const headers = item.rows.filter((r) => r.header);
      const placeRow = (r: RowLayout, rowIdx: number) => {
        const x0 = colX();
        for (const c of r.cells) {
          const cx = x0 + c.x;
          if (c.shading) page.decos.push({ kind: "rect", x: cx, y, w: c.w, h: r.height, fill: `#${c.shading}` });
          if (item.borders) {
            page.decos.push({ kind: "line", x: cx, y, w: 0.75, h: r.height, stroke: "#6b7280" });
            page.decos.push({ kind: "line", x: cx + c.w, y, w: 0.75, h: r.height, stroke: "#6b7280" });
            if (c.vMerge !== "continue") page.decos.push({ kind: "line", x: cx, y, w: c.w, h: 0.75, stroke: "#6b7280" });
            const nextRow = item.rows[rowIdx + 1];
            const below = nextRow?.cells.find((nc) => Math.abs(nc.x - c.x) < 0.5);
            if (!(below?.vMerge === "continue")) page.decos.push({ kind: "line", x: cx, y: y + r.height, w: c.w, h: 0.75, stroke: "#6b7280" });
          }
          page.areas.push({ container: c.container, x: cx, y, w: c.w, h: r.height });
          for (const l of c.lines) page.lines.push(toLine(l, cx + CELL_PAD, y + 1 + (l as FlowLine & { y: number }).y));
          for (const bx of c.boxes) page.boxes.push({ block: -1, y: y + bx.y, height: bx.height, left: cx + CELL_PAD, right: cx + c.w - CELL_PAD, label: bx.label });
        }
        y += r.height;
      };
      item.rows.forEach((r, ri) => {
        if (y + r.height > bottom() && !atTop()) {
          nextColumn();
          if (!r.header) headers.forEach((h) => placeRow(h, item.rows.indexOf(h)));
        }
        placeRow(r, ri);
      });
      y += 4;
      return;
    }
    const lines = item.lines;
    const paraH = lines.reduce((h, l) => h + l.height, 0);
    if (item.pageBreakBefore && !atTop()) newPage();
    const next = flow[idx + 1];
    if (item.keepNext && next) {
      const nextFirst = next.kind === "para" ? next.before + (next.lines[0]?.height ?? 0) : next.kind === "table" ? (next.rows[0]?.height ?? 0) : next.height;
      const need = item.before + paraH + item.after + nextFirst;
      if (y + need > bottom() && need <= height - margin.top - margin.bottom && !atTop()) nextColumn();
    }
    const top = atTop() ? 0 : item.before;
    if (lines.length >= 2 && y + top + lines[0]!.height <= bottom() && y + top + lines[0]!.height + lines[1]!.height > bottom() && !atTop()) nextColumn();
    y += atTop() ? 0 : item.before;

    lines.forEach((l, li) => {
      const refs = notesIn(l);
      const extra = refs.reduce((h, n) => h + (pageNotes.some((p) => p.kind === n.kind && p.id === n.id) ? 0 : noteFlow(n.kind, n.id).height), 0);
      const sepAdd = refs.length && !notesH ? NOTE_SEP : 0;
      if (y + l.height + extra + sepAdd > bottom() && !atTop()) {
        // widow control: never leave the last line alone at the top of a column
        const prev = page.lines[page.lines.length - 1];
        const moveBack = li === lines.length - 1 && li >= 2 && prev && prev.block === l.block && sameContainer(prev.container, null);
        if (moveBack) {
          page.lines.pop();
          nextColumn();
          const pl = lines[li - 1]!;
          page.lines.push(toLine(pl, colX(), y));
          y += pl.height;
        } else nextColumn();
      }
      for (const n of refs) {
        if (!pageNotes.some((p) => p.kind === n.kind && p.id === n.id)) {
          pageNotes.push(n);
          notesH += noteFlow(n.kind, n.id).height;
        }
      }
      page.lines.push(toLine(l, colX(), y));
      y += l.height;
      if (l.pageBreakAfter) newPage();
    });
    y += item.after;
    for (const l of lines) for (const f of l.frags) {
      if (f.kind !== "note") continue;
      const p = doc.blocks[l.block];
      const inl = p && isParagraph(p) ? findInlineAt(p, f.start) : null;
      if (inl?.type === "note" && inl.kind === "endnote" && !endnotes.some((e) => e.id === inl.id)) endnotes.push({ kind: "endnote", id: inl.id });
    }
  });

  // Endnotes flow after the body.
  if (endnotes.length) {
    if (y + 40 > bottom()) nextColumn();
    y += 12;
    page.decos.push({ kind: "line", x: colX(), y, w: Math.min(144, colW / 3), h: 0.75, stroke: "#9ca3af" });
    y += 8;
    for (const n of endnotes) {
      const f = noteFlow(n.kind, n.id);
      if (y + f.height > bottom() && !atTop()) nextColumn();
      page.areas.push({ container: { type: "note", kind: n.kind, id: n.id }, x: colX(), y, w: colW, h: f.height });
      for (const l of f.lines) page.lines.push(toLine(l, colX(), y + l.y));
      y += f.height;
    }
  }
  finishPage();

  // Column separators.
  if (nCols > 1 && s.columns?.separator) {
    for (const p of pages) for (let c = 1; c < nCols; c++) {
      const x = margin.left + c * (colW + gap) - gap / 2;
      p.decos.push({ kind: "line", x, y: margin.top, w: 0.75, h: height - margin.top - margin.bottom, stroke: "#9ca3af" });
    }
  }

  // Headers & footers (default variant), with page-number fields.
  const total = pages.length;
  (doc.headers ?? []).forEach((hf, index) => {
    if (hf.which !== "default") return;
    for (const p of pages) {
      const blocks = substituteFields(hf.blocks, p.index + 1, total);
      const st = layoutStatic({ ...env, ctx: { ...env.ctx, openComments: new Set() } }, blocks, contentW, () => ({ type: "header", index }));
      const top = hf.kind === "header" ? twipsToPx(s.marginHeader) : height - twipsToPx(s.marginFooter) - st.height;
      p.areas.push({ container: { type: "header", index }, x: margin.left, y: top, w: contentW, h: Math.max(st.height, 16) });
      for (const l of st.lines) p.lines.push({ ...toLine(l, margin.left, top + l.y), frags: l.frags.map((f) => ({ ...f, x: f.x + margin.left, style: { ...f.style, color: f.style.color } })) });
    }
  });

  return { pages, width, height, margin };
}

function findInlineAt(p: Paragraph, offset: number) {
  let at = 0;
  for (const i of p.inlines) {
    const len = inlineLength(i);
    if (offset >= at && offset < at + len) return i;
    at += len;
  }
  return null;
}

/** Deterministic measurer for tests: each char = 0.5 × font px size. */
export const monoMeasurer: Measurer = {
  width(text: string, font: string) {
    const px = Number(/([\d.]+)px/.exec(font)?.[1] ?? 16);
    return Array.from(text.replace(SHY, "")).length * px * 0.5;
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
        w = ctx.measureText(text.replace(SHY, "")).width;
        if (cache.size > 50_000) cache.clear();
        cache.set(key, w);
      }
      return w;
    },
  };
}

/** Clear layout caches (e.g. after embedded fonts load). */
export function resetLayoutCaches() {
  hyphenCache = new Map();
}

export type { Block };
