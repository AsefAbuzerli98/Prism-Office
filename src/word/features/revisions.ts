/** Tracked changes: recording deletions and accepting/rejecting (P2-17). */
import { isParagraph, isTable, type Block, type Inline, type Paragraph, type Revision } from "../model";
import { blockLength, comparePos, normalize, splitInlines, type Pos } from "../ops";

let seq = Date.now() % 100000;
export function newRevision(kind: Revision["kind"], author: string): Revision {
  return { kind, id: String(++seq), author, date: new Date().toISOString().replace(/\.\d+Z$/, "Z") };
}

/**
 * Delete [from, to) under track changes: the author's own insertions are
 * removed outright, other text is marked deleted, already-deleted text stays.
 * Paragraph boundaries are not merged.
 */
export function trackDelete(blocks: Block[], from: Pos, to: Pos, rev: Revision): Block[] {
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
    const changed: Inline[] = [];
    for (const i of mid) {
      if (i.type !== "text" && i.type !== "tab") {
        changed.push(i);
        continue;
      }
      const r = i.marks.revision;
      if (r?.kind === "insert" && r.author === rev.author) continue;
      if (r?.kind === "delete") changed.push(i);
      else changed.push({ ...i, marks: { ...i.marks, revision: rev } });
    }
    next[bi] = { ...b, inlines: normalize([...left, ...changed, ...right]) };
  }
  return next;
}

function resolveInlines(inlines: Inline[], accept: boolean, inRange: (offset: number) => boolean): Inline[] {
  const out: Inline[] = [];
  let at = 0;
  for (const i of inlines) {
    const len = i.type === "text" ? i.text.length : 1;
    const r = i.type === "text" || i.type === "tab" ? i.marks.revision : undefined;
    if (r && inRange(at) && (i.type === "text" || i.type === "tab")) {
      const keep = accept ? r.kind === "insert" : r.kind === "delete";
      if (keep) {
        const { revision: _r, ...marks } = i.marks;
        out.push({ ...i, marks });
      }
    } else out.push(i);
    at += len;
  }
  return normalize(out);
}

/** Accept or reject every tracked change in `blocks` (recursing into tables). */
export function resolveAll(blocks: Block[], accept: boolean): Block[] {
  return blocks.map((b) => {
    if (isParagraph(b)) return { ...b, inlines: resolveInlines(b.inlines, accept, () => true) };
    if (isTable(b)) return { ...b, rows: b.rows.map((r) => ({ ...r, cells: r.cells.map((c) => ({ ...c, blocks: resolveAll(c.blocks, accept) })) })) };
    return b;
  });
}

/** Accept or reject tracked changes intersecting [from, to]. */
export function resolveRange(blocks: Block[], from: Pos, to: Pos, accept: boolean): Block[] {
  if (comparePos(from, to) > 0) [from, to] = [to, from];
  return blocks.map((b, bi) => {
    if (bi < from.block || bi > to.block || !isParagraph(b)) return b;
    const s = bi === from.block ? from.offset : 0;
    const e = bi === to.block ? to.offset : blockLength(b);
    // a collapsed selection resolves the change under the caret
    const inRange = (o: number) => (s === e ? o <= s && o >= s - 1 : o >= s && o < e) || (s === e && o === s);
    return { ...b, inlines: resolveInlines(b.inlines, accept, inRange) } as Paragraph;
  });
}

export function hasRevisions(blocks: Block[]): boolean {
  return blocks.some((b) =>
    isParagraph(b)
      ? b.inlines.some((i) => (i.type === "text" || i.type === "tab") && !!i.marks.revision)
      : isTable(b) && b.rows.some((r) => r.cells.some((c) => hasRevisions(c.blocks))),
  );
}

/** Position of the next tracked change after `p` (wrapping), or null. */
export function nextRevision(blocks: Block[], p: Pos): Pos | null {
  const found: Pos[] = [];
  blocks.forEach((b, bi) => {
    if (!isParagraph(b)) return;
    let at = 0;
    for (const i of b.inlines) {
      if ((i.type === "text" || i.type === "tab") && i.marks.revision) found.push({ block: bi, offset: at });
      at += i.type === "text" ? i.text.length : 1;
    }
  });
  return found.find((f) => comparePos(f, p) > 0) ?? found[0] ?? null;
}
