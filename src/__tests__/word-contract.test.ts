import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { applyMarks, insertText, pos, splitParagraph, updateParagraphs } from "../word/ops";
import { blankDocument, BULLET_NUM_ID, newTable, type Paragraph, type WordDocument } from "../word/model";

/**
 * Contract between the TS editor and the Rust model: the document produced
 * here is checked in as a fixture that `crates/prism-word/tests/contract.rs`
 * deserialises and writes to DOCX. Run with UPDATE_FIXTURES=1 to regenerate.
 */
function editedDocument(): WordDocument {
  const doc = blankDocument();
  let r = insertText(doc.blocks, pos(0, 0), "Contract title", {});
  let blocks = updateParagraphs(r.blocks, pos(0, 0), pos(0, 0), (p) => ({ ...p, style: "Heading1", align: "center" }));
  r = splitParagraph(blocks, pos(0, 14), () => undefined);
  r = insertText(r.blocks, r.pos, "Body with bold\tand a link", {});
  blocks = applyMarks(r.blocks, pos(1, 10), pos(1, 14), { bold: true, color: "C00000", size: 28 });
  blocks = applyMarks(blocks, pos(1, 21), pos(1, 25), { link: "https://example.com" });
  r = splitParagraph(blocks, pos(1, 25), () => undefined);
  r = insertText(r.blocks, r.pos, "bullet", {});
  blocks = updateParagraphs(r.blocks, r.pos, r.pos, (p) => ({ ...p, list: { numId: BULLET_NUM_ID, level: 1 }, spaceAfter: 0, indentFirst: -360 }));
  const table = newTable(2, 2);
  table.rows[0]!.header = true;
  table.rows[0]!.cells[0]!.shading = "DDEEFF";
  table.rows[1]!.cells[1]!.blocks = [{ type: "paragraph", inlines: [{ type: "text", text: "cell", marks: { revision: { kind: "insert", id: "1", author: "Prism", date: "2026-11-12T00:00:00Z" } } }] }];
  const extras: Paragraph = {
    type: "paragraph",
    bidi: true,
    tabs: [{ pos: 9016, align: "right", leader: "." }],
    inlines: [
      { type: "comment", id: "0", edge: "start" },
      { type: "text", text: "noted", marks: { rtl: true } },
      { type: "comment", id: "0", edge: "end" },
      { type: "comment", id: "0", edge: "reference" },
      { type: "note", kind: "footnote", id: 1, xml: "" },
      { type: "shape", preset: "ellipse", cx: 914400, cy: 457200, stroke: "1F2937", xml: "" },
    ],
  };
  blocks = [...blocks, table, extras];
  return {
    ...doc,
    blocks,
    notes: [{ kind: "footnote", id: 1, blocks: [{ type: "paragraph", style: "FootnoteText", inlines: [{ type: "text", text: "A note", marks: {} }] }] }],
    comments: [{ id: "0", author: "Prism", initials: "P", blocks: [{ type: "paragraph", inlines: [{ type: "text", text: "Check", marks: {} }] }] }],
    sources: [{ tag: "Doe24", kind: "Report", title: "Docs", authors: ["Doe, J"], year: "2024", publisher: "", url: "" }],
    settings: { trackRevisions: true, autoHyphenation: false },
    section: { ...doc.section, landscape: true, pageWidth: 16838, pageHeight: 11906, columns: { count: 2, space: 720, separator: true } },
  };
}

describe("TS ⇄ Rust document contract", () => {
  it("matches the checked-in fixture", () => {
    const file = resolve(dirname(fileURLToPath(import.meta.url)), "../../crates/prism-word/tests/fixtures/ts-document.json");
    const json = JSON.stringify(editedDocument(), null, 2) + "\n";
    if (process.env.UPDATE_FIXTURES) writeFileSync(file, json);
    expect(readFileSync(file, "utf8")).toBe(json);
  });
});
