import { describe, expect, it } from "vitest";
import { containerBlocks, pruneOrphans, withContainerBlocks } from "../word/containers";
import { buildBibliography, buildToc, citationField, citationText, findToc, headings, instrOf, updateFields } from "../word/features/fields";
import { hasRevisions, nextRevision, resolveAll, resolveRange, trackDelete } from "../word/features/revisions";
import { deleteColumn, deleteRow, insertColumn, insertRow, mergeRight, splitCell, toggleHeaderRow } from "../word/features/tables";
import { blankDocument, newTable, paragraph, type Paragraph, type WordDocument } from "../word/model";
import { paragraphText, pos } from "../word/ops";

const P = (b: unknown) => b as Paragraph;

describe("containers", () => {
  it("reads and replaces cell/note/header content immutably", () => {
    const doc: WordDocument = {
      ...blankDocument(),
      blocks: [paragraph("x"), newTable(2, 2)],
      notes: [{ kind: "footnote", id: 1, blocks: [paragraph("n")] }],
      headers: [{ kind: "header", which: "default", relId: "rId9", blocks: [paragraph("h")] }],
    };
    const cell = { type: "cell" as const, table: 1, row: 1, col: 0 };
    const d2 = withContainerBlocks(doc, cell, [paragraph("in cell")]);
    expect(paragraphText(P(containerBlocks(d2, cell)[0]))).toBe("in cell");
    expect(paragraphText(P(containerBlocks(doc, cell)[0]))).toBe("");
    const d3 = withContainerBlocks(d2, { type: "note", kind: "footnote", id: 1 }, [paragraph("note!")]);
    expect(paragraphText(P(d3.notes![0]!.blocks[0]))).toBe("note!");
    const d4 = withContainerBlocks(d3, { type: "header", index: 0 }, [paragraph("H")]);
    expect(paragraphText(P(d4.headers![0]!.blocks[0]))).toBe("H");
  });

  it("prunes notes and comments without references", () => {
    const doc: WordDocument = {
      ...blankDocument(),
      blocks: [{ type: "paragraph", inlines: [{ type: "note", kind: "footnote", id: 2, xml: "" }] }],
      notes: [{ kind: "footnote", id: 1, blocks: [] }, { kind: "footnote", id: 2, blocks: [] }],
      comments: [{ id: "c", author: "a", initials: "a", blocks: [] }],
    };
    const d = pruneOrphans(doc);
    expect(d.notes!.map((n) => n.id)).toEqual([2]);
    expect(d.comments).toEqual([]);
  });
});

describe("table edits", () => {
  it("inserts/deletes rows and columns, merges and splits", () => {
    let t = newTable(2, 2);
    t = insertRow(t, 0, true);
    expect(t.rows).toHaveLength(3);
    t = insertColumn(t, 1, true);
    expect(t.rows.every((r) => r.cells.length === 3)).toBe(true);
    expect(t.grid).toHaveLength(3);
    expect(Math.abs(t.grid.reduce((a, b) => a + b, 0) - 9026)).toBeLessThan(5);
    t = mergeRight(t, 0, 0);
    expect(t.rows[0]!.cells[0]!.span).toBe(2);
    expect(t.rows[0]!.cells).toHaveLength(2);
    t = splitCell(t, 0, 0);
    expect(t.rows[0]!.cells).toHaveLength(3);
    t = deleteColumn(t, 2)!;
    t = deleteRow(t, 0)!;
    expect([t.rows.length, t.rows[0]!.cells.length]).toEqual([2, 2]);
    expect(deleteRow(deleteRow(t, 0)!, 0)).toBeNull();
    expect(toggleHeaderRow(t).rows[0]!.header).toBe(true);
  });
});

describe("revisions", () => {
  const rev = { kind: "delete" as const, id: "9", author: "Me" };
  it("tracked delete marks others' text and removes own insertions", () => {
    const p: Paragraph = {
      type: "paragraph",
      inlines: [
        { type: "text", text: "keep ", marks: {} },
        { type: "text", text: "mine", marks: { revision: { kind: "insert", id: "1", author: "Me" } } },
        { type: "text", text: "theirs", marks: { revision: { kind: "insert", id: "2", author: "You" } } },
      ],
    };
    const out = P(trackDelete([p], pos(0, 2), pos(0, 15), rev)[0]);
    expect(paragraphText(out)).toBe("keep theirs");
    expect(out.inlines.map((i) => (i.type === "text" ? i.marks.revision?.kind ?? "-" : "?"))).toEqual(["-", "delete"]);
    expect(hasRevisions([out])).toBe(true);
    expect(nextRevision([out], pos(0, 0))).toEqual(pos(0, 2));
    expect(paragraphText(P(resolveAll([out], true)[0]))).toBe("ke");
    expect(paragraphText(P(resolveAll([out], false)[0]))).toBe("keep theirs");
    expect(hasRevisions(resolveAll([out], false))).toBe(false);
    const partly = P(resolveRange([out], pos(0, 0), pos(0, 3), true)[0]);
    expect(paragraphText(partly)).toBe("ke");
  });
});

describe("fields", () => {
  const doc: WordDocument = {
    ...blankDocument(),
    blocks: [paragraph("Intro", { style: "Heading1" }), paragraph("body"), paragraph("Detail", { style: "Heading2" })],
    sources: [{ tag: "Smi20", kind: "Book", title: "Rust", authors: ["Smith, Jane", "Doe, J"], year: "2020", publisher: "Pub", url: "" }],
  };

  it("builds and updates a table of contents", () => {
    const hs = headings(doc);
    expect(hs.map((h) => [h.level, h.text])).toEqual([[1, "Intro"], [2, "Detail"]]);
    const toc = buildToc(hs, (b) => b + 1);
    expect(toc.map((b) => paragraphText(P(b)).replace(/￼/g, ""))).toEqual(["Contents", "Intro\t1", "Detail\t3"]);
    expect(instrOf(P(toc[1]).inlines[1]!)).toContain("TOC");
    const withToc = { ...doc, blocks: [...toc, ...doc.blocks] };
    expect(findToc(withToc.blocks)).toEqual([1, 2]);
    const renamed = { ...withToc, blocks: withToc.blocks.map((b, i) => (i === 3 ? paragraph("Introduction", { style: "Heading1" }) : b)) };
    const updated = updateFields(renamed, () => 7);
    expect(paragraphText(P(updated.blocks[1])).replace(/￼/g, "")).toBe("Introduction\t7");
    expect(updated.blocks).toHaveLength(renamed.blocks.length);
  });

  it("formats citations and the bibliography and refreshes them", () => {
    expect(citationText(doc.sources![0], "Smi20")).toBe("(Smith & Doe, 2020)");
    const cite: Paragraph = { type: "paragraph", inlines: citationField(undefined, "Smi20") };
    const bib = buildBibliography([]);
    const d = updateFields({ ...doc, blocks: [cite, ...bib] }, () => 1);
    expect(paragraphText(P(d.blocks[0])).replace(/￼/g, "")).toBe("(Smith & Doe, 2020)");
    expect(paragraphText(P(d.blocks[1]))).toBe("References");
    expect(paragraphText(P(d.blocks[2]))).toContain("Smith, J., Doe, J. (2020). Rust. Pub.");
  });
});
