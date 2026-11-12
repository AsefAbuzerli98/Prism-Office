import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { applyMarks, insertText, pos, splitParagraph, updateParagraphs } from "../word/ops";
import { blankDocument, BULLET_NUM_ID, type WordDocument } from "../word/model";

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
  blocks = [...blocks, { type: "opaque", label: "Table", xml: '<w:tbl xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl>' }];
  return { ...doc, blocks, section: { ...doc.section, landscape: true, pageWidth: 16838, pageHeight: 11906 } };
}

describe("TS ⇄ Rust document contract", () => {
  it("matches the checked-in fixture", () => {
    const file = resolve(dirname(fileURLToPath(import.meta.url)), "../../crates/prism-word/tests/fixtures/ts-document.json");
    const json = JSON.stringify(editedDocument(), null, 2) + "\n";
    if (process.env.UPDATE_FIXTURES) writeFileSync(file, json);
    expect(readFileSync(file, "utf8")).toBe(json);
  });
});
