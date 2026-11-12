/** Addressing editable block containers: body, table cells, notes, headers. */
import type { ContainerRef } from "./layout";
import { isParagraph, isTable, type Block, type Inline, type WordDocument } from "./model";

export function containerBlocks(doc: WordDocument, c: ContainerRef | null): Block[] {
  if (!c) return doc.blocks;
  switch (c.type) {
    case "cell": {
      const t = doc.blocks[c.table];
      return (t && isTable(t) && t.rows[c.row]?.cells[c.col]?.blocks) || [];
    }
    case "note":
      return doc.notes?.find((n) => n.kind === c.kind && n.id === c.id)?.blocks ?? [];
    case "header":
      return doc.headers?.[c.index]?.blocks ?? [];
  }
}

export function withContainerBlocks(doc: WordDocument, c: ContainerRef | null, blocks: Block[]): WordDocument {
  if (!c) return { ...doc, blocks };
  switch (c.type) {
    case "cell": {
      const t = doc.blocks[c.table];
      if (!t || !isTable(t)) return doc;
      const rows = t.rows.map((r, ri) =>
        ri !== c.row ? r : { ...r, cells: r.cells.map((cell, ci) => (ci !== c.col ? cell : { ...cell, blocks: blocks.length ? blocks : [{ type: "paragraph" as const, inlines: [] }] })) },
      );
      const next = doc.blocks.slice();
      next[c.table] = { ...t, rows };
      return { ...doc, blocks: next };
    }
    case "note":
      return { ...doc, notes: (doc.notes ?? []).map((n) => (n.kind === c.kind && n.id === c.id ? { ...n, blocks } : n)) };
    case "header":
      return { ...doc, headers: (doc.headers ?? []).map((h, i) => (i === c.index ? { ...h, blocks } : h)) };
  }
}

/** Visit every inline in the document (body, cells, notes, headers, comments). */
export function walkInlines(doc: WordDocument, fn: (i: Inline) => void) {
  const blocks = (bs: Block[]) => {
    for (const b of bs) {
      if (isParagraph(b)) b.inlines.forEach(fn);
      else if (isTable(b)) b.rows.forEach((r) => r.cells.forEach((c) => blocks(c.blocks)));
    }
  };
  blocks(doc.blocks);
  (doc.notes ?? []).forEach((n) => blocks(n.blocks));
  (doc.headers ?? []).forEach((h) => blocks(h.blocks));
}

/** Drop notes and comments whose reference marks were deleted. */
export function pruneOrphans(doc: WordDocument): WordDocument {
  const notes = new Set<string>();
  const comments = new Set<string>();
  walkInlines(doc, (i) => {
    if (i.type === "note") notes.add(`${i.kind}:${i.id}`);
    if (i.type === "comment") comments.add(i.id);
  });
  const keptNotes = (doc.notes ?? []).filter((n) => notes.has(`${n.kind}:${n.id}`));
  const keptComments = (doc.comments ?? []).filter((c) => comments.has(c.id));
  if (keptNotes.length === (doc.notes ?? []).length && keptComments.length === (doc.comments ?? []).length) return doc;
  return { ...doc, notes: keptNotes, comments: keptComments };
}

/** Map every block array in the document through `fn` (recursing into cells). */
export function mapAllBlocks(doc: WordDocument, fn: (blocks: Block[]) => Block[]): WordDocument {
  const rec = (bs: Block[]): Block[] =>
    fn(bs).map((b) => (isTable(b) ? { ...b, rows: b.rows.map((r) => ({ ...r, cells: r.cells.map((c) => ({ ...c, blocks: rec(c.blocks) })) })) } : b));
  return {
    ...doc,
    blocks: rec(doc.blocks),
    notes: doc.notes?.map((n) => ({ ...n, blocks: rec(n.blocks) })),
    headers: doc.headers?.map((h) => ({ ...h, blocks: rec(h.blocks) })),
  };
}
