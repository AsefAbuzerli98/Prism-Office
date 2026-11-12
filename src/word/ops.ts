/**
 * Pure, immutable editing operations on the document model.
 *
 * Positions address a paragraph block and an offset into its *flattened*
 * content where every text code unit counts 1 and every non-text inline
 * (tab, break, opaque) counts 1. Opaque blocks have length 1 (offset 0|1).
 * Operations return new block arrays and never mutate their input, so the
 * history can keep cheap structurally-shared snapshots.
 */
import { isParagraph, type Block, type Inline, type Marks, type Paragraph } from "./model";

export interface Pos {
  block: number;
  offset: number;
}

export interface Selection {
  anchor: Pos;
  focus: Pos;
}

export const pos = (block: number, offset: number): Pos => ({ block, offset });

export function comparePos(a: Pos, b: Pos): number {
  return a.block - b.block || a.offset - b.offset;
}

export function ordered(sel: Selection): [Pos, Pos] {
  return comparePos(sel.anchor, sel.focus) <= 0 ? [sel.anchor, sel.focus] : [sel.focus, sel.anchor];
}

export const isCollapsed = (sel: Selection) => comparePos(sel.anchor, sel.focus) === 0;

export function inlineLength(i: Inline): number {
  return i.type === "text" ? i.text.length : 1;
}

export function blockLength(b: Block): number {
  return isParagraph(b) ? b.inlines.reduce((n, i) => n + inlineLength(i), 0) : 1;
}

/** Visible text of a paragraph; non-text inlines map to single characters. */
export function paragraphText(p: Paragraph): string {
  return p.inlines
    .map((i) => (i.type === "text" ? i.text : i.type === "tab" ? "\t" : i.type === "break" ? "\n" : "￼"))
    .join("");
}

export function marksEqual(a: Marks, b: Marks): boolean {
  const ka = Object.keys(a).filter((k) => a[k as keyof Marks] !== undefined);
  const kb = Object.keys(b).filter((k) => b[k as keyof Marks] !== undefined);
  if (ka.length !== kb.length) return false;
  return ka.every((k) => JSON.stringify(a[k as keyof Marks]) === JSON.stringify(b[k as keyof Marks]));
}

/** Merge adjacent text inlines with equal marks; drop empty text. */
export function normalize(inlines: Inline[]): Inline[] {
  const out: Inline[] = [];
  for (const i of inlines) {
    if (i.type === "text" && i.text === "") continue;
    const last = out[out.length - 1];
    if (i.type === "text" && last?.type === "text" && marksEqual(last.marks, i.marks)) {
      out[out.length - 1] = { ...last, text: last.text + i.text };
    } else {
      out.push(i);
    }
  }
  return out;
}

/** Split inlines at a flattened offset. */
export function splitInlines(inlines: Inline[], offset: number): [Inline[], Inline[]] {
  const left: Inline[] = [];
  const right: Inline[] = [];
  let at = 0;
  for (const i of inlines) {
    const len = inlineLength(i);
    if (at + len <= offset) left.push(i);
    else if (at >= offset) right.push(i);
    else if (i.type === "text") {
      const cut = offset - at;
      left.push({ ...i, text: i.text.slice(0, cut) });
      right.push({ ...i, text: i.text.slice(cut) });
    }
    at += len;
  }
  return [left, right];
}

/** Marks in effect for typing at `offset`: those of the preceding character,
 * or of the first character when at the paragraph start. */
export function marksAt(p: Paragraph, offset: number): Marks {
  let at = 0;
  let first: Marks | null = null;
  for (const i of p.inlines) {
    const len = inlineLength(i);
    if (i.type === "text" || i.type === "tab") {
      first ??= i.marks;
      if (offset > at && offset <= at + len) return i.marks;
    }
    at += len;
  }
  return first ?? {};
}

function withInlines(p: Paragraph, inlines: Inline[]): Paragraph {
  return { ...p, inlines: normalize(inlines) };
}

function textToInlines(text: string, marks: Marks): Inline[] {
  const out: Inline[] = [];
  let buf = "";
  for (const ch of text) {
    if (ch === "\t") {
      if (buf) out.push({ type: "text", text: buf, marks });
      buf = "";
      out.push({ type: "tab", marks });
    } else if (ch === "\u000b" || ch === " ") {
      if (buf) out.push({ type: "text", text: buf, marks });
      buf = "";
      out.push({ type: "break", kind: "line" });
    } else buf += ch;
  }
  if (buf) out.push({ type: "text", text: buf, marks });
  return out;
}

/** Paragraph properties carried to a paragraph created by Enter. */
export function continuation(p: Paragraph, nextStyle?: string): Paragraph {
  const { inlines: _drop, ...props } = p;
  const next: Paragraph = { ...props, inlines: [] };
  if (nextStyle !== undefined) next.style = nextStyle === "Normal" ? undefined : nextStyle;
  next.pageBreakBefore = undefined;
  return next;
}

/** Remove the content between two positions, merging paragraphs. */
export function deleteRange(blocks: Block[], from: Pos, to: Pos): { blocks: Block[]; pos: Pos } {
  if (comparePos(from, to) > 0) [from, to] = [to, from];
  if (comparePos(from, to) === 0) return { blocks, pos: from };
  const a = blocks[from.block]!;
  const b = blocks[to.block]!;
  const next = blocks.slice();
  if (from.block === to.block) {
    if (!isParagraph(a)) {
      next.splice(from.block, 1, ...(next.length === 1 ? [emptyLike()] : []));
      return { blocks: next, pos: { block: Math.min(from.block, next.length - 1), offset: 0 } };
    }
    const [left] = splitInlines(a.inlines, from.offset);
    const [, right] = splitInlines(a.inlines, to.offset);
    next[from.block] = withInlines(a, [...left, ...right]);
    return { blocks: next, pos: from };
  }
  // Multi-block: keep head of `a` (or drop if opaque), tail of `b`.
  const head = isParagraph(a) ? splitInlines(a.inlines, from.offset)[0] : null;
  const tail = isParagraph(b) ? splitInlines(b.inlines, to.offset)[1] : null;
  let merged: Block[];
  if (isParagraph(a)) {
    merged = [withInlines(a, [...head!, ...(tail ?? [])])];
    if (!isParagraph(b) && to.offset === 0) merged.push(b);
  } else if (isParagraph(b)) {
    merged = from.offset === 0 ? [withInlines(b, tail!)] : [a, withInlines(b, tail!)];
  } else {
    merged = [...(from.offset === 0 ? [] : [a]), ...(to.offset === 0 ? [b] : [])];
    if (merged.length === 0) merged = [emptyLike()];
  }
  next.splice(from.block, to.block - from.block + 1, ...merged);
  const block = Math.min(from.block, next.length - 1);
  const offset = isParagraph(a) ? from.offset : 0;
  return { blocks: next, pos: { block, offset: isParagraph(next[block]!) ? offset : 0 } };
}

function emptyLike(): Paragraph {
  return { type: "paragraph", inlines: [] };
}

/** Insert text at `at` with `marks`; `\n` splits paragraphs. */
export function insertText(
  blocks: Block[],
  at: Pos,
  text: string,
  marks: Marks,
  nextStyleOf: (style: string | undefined) => string | undefined = () => undefined,
): { blocks: Block[]; pos: Pos } {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  let cur = { blocks, pos: at };
  lines.forEach((line, idx) => {
    if (idx > 0) cur = splitParagraph(cur.blocks, cur.pos, nextStyleOf);
    if (!line) return;
    const b = cur.blocks[cur.pos.block]!;
    if (!isParagraph(b)) {
      // Typing on an opaque block inserts a new paragraph after it.
      const next = cur.blocks.slice();
      next.splice(cur.pos.block + 1, 0, { type: "paragraph", inlines: textToInlines(line, marks) });
      cur = { blocks: next, pos: { block: cur.pos.block + 1, offset: line.length } };
      return;
    }
    const [left, right] = splitInlines(b.inlines, cur.pos.offset);
    const added = textToInlines(line, marks);
    const next = cur.blocks.slice();
    next[cur.pos.block] = withInlines(b, [...left, ...added, ...right]);
    const len = added.reduce((n, i) => n + inlineLength(i), 0);
    cur = { blocks: next, pos: { block: cur.pos.block, offset: cur.pos.offset + len } };
  });
  return cur;
}

export function insertInline(blocks: Block[], at: Pos, inline: Inline): { blocks: Block[]; pos: Pos } {
  const b = blocks[at.block]!;
  if (!isParagraph(b)) return { blocks, pos: at };
  const [left, right] = splitInlines(b.inlines, at.offset);
  const next = blocks.slice();
  next[at.block] = withInlines(b, [...left, inline, ...right]);
  return { blocks: next, pos: { block: at.block, offset: at.offset + 1 } };
}

export function splitParagraph(
  blocks: Block[],
  at: Pos,
  nextStyleOf: (style: string | undefined) => string | undefined = () => undefined,
): { blocks: Block[]; pos: Pos } {
  const b = blocks[at.block]!;
  const next = blocks.slice();
  if (!isParagraph(b)) {
    next.splice(at.block + at.offset, 0, emptyLike());
    return { blocks: next, pos: { block: at.block + at.offset, offset: 0 } };
  }
  const [left, right] = splitInlines(b.inlines, at.offset);
  // Enter at the end of a heading continues with its `next` style.
  const atEnd = right.length === 0;
  const tail = continuation(b, atEnd ? nextStyleOf(b.style) : undefined);
  next.splice(at.block, 1, withInlines(b, left), { ...tail, inlines: normalize(right) });
  return { blocks: next, pos: { block: at.block + 1, offset: 0 } };
}

/** Apply `patch` to all text/tab marks within [from, to). `undefined` values remove a mark. */
export function applyMarks(blocks: Block[], from: Pos, to: Pos, patch: Partial<Marks>): Block[] {
  if (comparePos(from, to) > 0) [from, to] = [to, from];
  const next = blocks.slice();
  for (let bi = from.block; bi <= to.block; bi++) {
    const b = blocks[bi]!;
    if (!isParagraph(b)) continue;
    const start = bi === from.block ? from.offset : 0;
    const end = bi === to.block ? to.offset : blockLength(b);
    if (start >= end) continue;
    const [left, rest] = splitInlines(b.inlines, start);
    const [mid, right] = splitInlines(rest, end - start);
    const patched = mid.map((i) => {
      if (i.type !== "text" && i.type !== "tab") return i;
      const marks: Marks = { ...i.marks, ...patch };
      for (const k of Object.keys(marks) as (keyof Marks)[]) if (marks[k] === undefined) delete marks[k];
      return { ...i, marks };
    });
    next[bi] = withInlines(b, [...left, ...patched, ...right]);
  }
  return next;
}

/** Whether every character in the range has `mark` set truthy (for toggles). */
export function rangeHasMark(blocks: Block[], from: Pos, to: Pos, mark: keyof Marks): boolean {
  if (comparePos(from, to) > 0) [from, to] = [to, from];
  let any = false;
  for (let bi = from.block; bi <= to.block; bi++) {
    const b = blocks[bi]!;
    if (!isParagraph(b)) continue;
    const start = bi === from.block ? from.offset : 0;
    const end = bi === to.block ? to.offset : blockLength(b);
    const [, rest] = splitInlines(b.inlines, start);
    const [mid] = splitInlines(rest, end - start);
    for (const i of mid) {
      if (i.type !== "text") continue;
      any = true;
      if (!i.marks[mark]) return false;
    }
  }
  return any;
}

/** Update paragraph-level properties for every paragraph touched by the range. */
export function updateParagraphs(
  blocks: Block[],
  from: Pos,
  to: Pos,
  fn: (p: Paragraph) => Paragraph,
): Block[] {
  const [a, b] = comparePos(from, to) <= 0 ? [from, to] : [to, from];
  const next = blocks.slice();
  for (let bi = a.block; bi <= b.block; bi++) {
    const blk = blocks[bi]!;
    if (isParagraph(blk)) next[bi] = fn(blk);
  }
  return next;
}

/** Extract the blocks covered by a range (for copy). */
export function sliceBlocks(blocks: Block[], from: Pos, to: Pos): Block[] {
  if (comparePos(from, to) > 0) [from, to] = [to, from];
  const out: Block[] = [];
  for (let bi = from.block; bi <= to.block; bi++) {
    const b = blocks[bi]!;
    if (!isParagraph(b)) {
      if (!(bi === to.block && to.offset === 0)) out.push(b);
      continue;
    }
    const start = bi === from.block ? from.offset : 0;
    const end = bi === to.block ? to.offset : blockLength(b);
    const [, rest] = splitInlines(b.inlines, start);
    const [mid] = splitInlines(rest, end - start);
    out.push({ ...b, inlines: mid });
  }
  return out;
}

/** Insert copied blocks at a position (paste). */
export function insertBlocks(blocks: Block[], at: Pos, frag: Block[]): { blocks: Block[]; pos: Pos } {
  if (frag.length === 0) return { blocks, pos: at };
  const target = blocks[at.block]!;
  if (frag.length === 1 && isParagraph(frag[0]!) && isParagraph(target)) {
    const [left, right] = splitInlines(target.inlines, at.offset);
    const ins = frag[0]!.inlines;
    const next = blocks.slice();
    next[at.block] = withInlines(target, [...left, ...ins, ...right]);
    return { blocks: next, pos: { block: at.block, offset: at.offset + ins.reduce((n, i) => n + inlineLength(i), 0) } };
  }
  // Multi-block: split target, put first frag paragraph's inlines into head, last into tail.
  let { blocks: cur, pos: p } = splitParagraph(blocks, at);
  const head = cur[p.block - 1]!;
  const tail = cur[p.block]!;
  const first = frag[0]!;
  const last = frag[frag.length - 1]!;
  const middle = frag.slice(1, -1);
  const out = cur.slice();
  const repl: Block[] = [];
  if (isParagraph(head) && isParagraph(first)) {
    repl.push(withInlines(head, [...head.inlines, ...first.inlines]));
  } else {
    repl.push(head, first);
  }
  repl.push(...middle);
  let caret: Pos;
  if (frag.length > 1 && isParagraph(tail) && isParagraph(last)) {
    const lastLen = blockLength(last);
    repl.push(withInlines({ ...last }, [...last.inlines, ...tail.inlines]));
    caret = { block: p.block - 1 + repl.length - 1, offset: lastLen };
  } else {
    if (frag.length > 1) repl.push(last);
    repl.push(tail);
    caret = { block: p.block - 1 + repl.length - 1, offset: 0 };
  }
  out.splice(p.block - 1, 2, ...repl);
  cur = out;
  p = caret;
  return { blocks: cur, pos: p };
}

/** Clamp a position into the document. */
export function clampPos(blocks: Block[], p: Pos): Pos {
  const block = Math.max(0, Math.min(p.block, blocks.length - 1));
  const len = blockLength(blocks[block]!);
  return { block, offset: Math.max(0, Math.min(p.offset, len)) };
}

/** Step one character (code point, skipping surrogate halves) left/right across blocks. */
export function stepPos(blocks: Block[], p: Pos, dir: 1 | -1): Pos {
  const b = blocks[p.block]!;
  const len = blockLength(b);
  if (dir < 0) {
    if (p.offset > 0) {
      let o = p.offset - 1;
      if (isParagraph(b) && isLowSurrogateAt(b, o)) o--;
      return { block: p.block, offset: o };
    }
    return p.block > 0 ? { block: p.block - 1, offset: blockLength(blocks[p.block - 1]!) } : p;
  }
  if (p.offset < len) {
    let o = p.offset + 1;
    if (isParagraph(b) && isLowSurrogateAt(b, o)) o++;
    return { block: p.block, offset: o };
  }
  return p.block < blocks.length - 1 ? { block: p.block + 1, offset: 0 } : p;
}

function isLowSurrogateAt(p: Paragraph, offset: number): boolean {
  const c = paragraphText(p).charCodeAt(offset);
  return c >= 0xdc00 && c <= 0xdfff;
}

/** Word boundary search within a paragraph (Ctrl/Alt + arrows, word delete). */
export function wordBoundary(blocks: Block[], p: Pos, dir: 1 | -1): Pos {
  const b = blocks[p.block]!;
  if (!isParagraph(b)) return stepPos(blocks, p, dir);
  const text = paragraphText(b);
  const isWord = (c: string | undefined) => !!c && /[\p{L}\p{N}_]/u.test(c);
  let o = p.offset;
  if (dir < 0) {
    if (o === 0) return stepPos(blocks, p, -1);
    while (o > 0 && !isWord(text[o - 1])) o--;
    while (o > 0 && isWord(text[o - 1])) o--;
  } else {
    if (o === text.length) return stepPos(blocks, p, 1);
    while (o < text.length && !isWord(text[o])) o++;
    while (o < text.length && isWord(text[o])) o++;
  }
  return { block: p.block, offset: o };
}
