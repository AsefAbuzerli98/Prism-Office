import { describe, expect, it } from "vitest";
import { History } from "../word/history";
import { blockLabel } from "../word/model";
import { paragraph, type Block, type Paragraph } from "../word/model";
import {
  applyMarks, blockLength, deleteRange, insertBlocks, insertText, marksAt, normalize, paragraphText,
  pos, rangeHasMark, sliceBlocks, splitParagraph, stepPos, updateParagraphs, wordBoundary,
} from "../word/ops";

const texts = (blocks: Block[]) => blocks.map((b) => (b.type === "paragraph" ? paragraphText(b) : `[${blockLabel(b)}]`));
const P = (b: Block) => b as Paragraph;

describe("word ops", () => {
  it("inserts text, tabs and paragraph breaks", () => {
    const r = insertText([paragraph("Hello")], pos(0, 5), " world\tx\nnext", {});
    expect(texts(r.blocks)).toEqual(["Hello world\tx", "next"]);
    expect(r.pos).toEqual(pos(1, 4));
    expect(P(r.blocks[0]!).inlines.map((i) => i.type)).toEqual(["text", "tab", "text"]);
  });

  it("deletes within and across paragraphs, merging", () => {
    const blocks = [paragraph("abcdef"), paragraph("ghij"), paragraph("klm")];
    expect(texts(deleteRange(blocks, pos(0, 1), pos(0, 3)).blocks)).toEqual(["adef", "ghij", "klm"]);
    const r = deleteRange(blocks, pos(0, 4), pos(2, 1));
    expect(texts(r.blocks)).toEqual(["abcdlm"]);
    expect(r.pos).toEqual(pos(0, 4));
    expect(blocks).toHaveLength(3); // input untouched
  });

  it("deletes opaque blocks as single units", () => {
    const blocks: Block[] = [paragraph("a"), { type: "opaque", label: "Table", xml: "<t/>" }, paragraph("b")];
    expect(texts(deleteRange(blocks, pos(1, 0), pos(1, 1)).blocks)).toEqual(["a", "b"]);
    expect(texts(deleteRange(blocks, pos(0, 1), pos(2, 0)).blocks)).toEqual(["ab"]);
  });

  it("splits paragraphs keeping properties and applying next style at end", () => {
    const h = paragraph("Title", { style: "Heading1", align: "center" });
    const next = (s?: string) => (s === "Heading1" ? "Normal" : undefined);
    const atEnd = splitParagraph([h], pos(0, 5), next);
    expect(P(atEnd.blocks[1]!).style).toBeUndefined();
    expect(P(atEnd.blocks[1]!).align).toBe("center");
    const mid = splitParagraph([h], pos(0, 2), next);
    expect(texts(mid.blocks)).toEqual(["Ti", "tle"]);
    expect(P(mid.blocks[1]!).style).toBe("Heading1");
  });

  it("applies and toggles marks over ranges", () => {
    const blocks = [paragraph("hello world"), paragraph("second")];
    const bold = applyMarks(blocks, pos(0, 6), pos(1, 3), { bold: true });
    expect(P(bold[0]!).inlines).toEqual([
      { type: "text", text: "hello ", marks: {} },
      { type: "text", text: "world", marks: { bold: true } },
    ]);
    expect(rangeHasMark(bold, pos(0, 6), pos(1, 3), "bold")).toBe(true);
    expect(rangeHasMark(bold, pos(0, 0), pos(1, 3), "bold")).toBe(false);
    const unbold = applyMarks(bold, pos(0, 0), pos(1, 6), { bold: undefined });
    expect(P(unbold[0]!).inlines).toEqual([{ type: "text", text: "hello world", marks: {} }]);
  });

  it("typing inherits marks of the preceding character", () => {
    const p = P(applyMarks([paragraph("ab")], pos(0, 1), pos(0, 2), { italic: true })[0]!);
    expect(marksAt(p, 2)).toEqual({ italic: true });
    expect(marksAt(p, 1)).toEqual({});
    expect(marksAt(p, 0)).toEqual({});
  });

  it("normalizes adjacent runs", () => {
    expect(normalize([{ type: "text", text: "a", marks: { bold: true } }, { type: "text", text: "", marks: {} }, { type: "text", text: "b", marks: { bold: true } }]))
      .toEqual([{ type: "text", text: "ab", marks: { bold: true } }]);
  });

  it("slices and re-inserts fragments (copy/paste)", () => {
    const blocks = [paragraph("one two"), paragraph("three"), paragraph("four")];
    const frag = sliceBlocks(blocks, pos(0, 4), pos(1, 2));
    expect(texts(frag)).toEqual(["two", "th"]);
    const r = insertBlocks([paragraph("XY")], pos(0, 1), frag);
    expect(texts(r.blocks)).toEqual(["Xtwo", "thY"]);
    expect(r.pos).toEqual(pos(1, 2));
    const single = insertBlocks([paragraph("XY")], pos(0, 1), [paragraph("--")]);
    expect(texts(single.blocks)).toEqual(["X--Y"]);
  });

  it("updates paragraph properties over a range", () => {
    const r = updateParagraphs([paragraph("a"), paragraph("b"), paragraph("c")], pos(2, 0), pos(1, 0), (p) => ({ ...p, align: "right" }));
    expect(r.map((b) => P(b).align)).toEqual([undefined, "right", "right"]);
  });

  it("steps by code point and by word", () => {
    const blocks = [paragraph("a😀b"), paragraph("x")];
    expect(stepPos(blocks, pos(0, 1), 1)).toEqual(pos(0, 3));
    expect(stepPos(blocks, pos(0, 3), -1)).toEqual(pos(0, 1));
    expect(stepPos(blocks, pos(0, 4), 1)).toEqual(pos(1, 0));
    const w = [paragraph("Salam, dünya gözəl")];
    expect(wordBoundary(w, pos(0, 0), 1)).toEqual(pos(0, 5));
    expect(wordBoundary(w, pos(0, 5), 1)).toEqual(pos(0, 12));
    expect(wordBoundary(w, pos(0, 18), -1)).toEqual(pos(0, 13));
    expect(blockLength(w[0]!)).toBe(18);
  });
});

describe("History", () => {
  it("groups typing, seals on demand, supports redo", () => {
    let t = 0;
    const h = new History(100, 1000, () => t);
    const s = (n: number) => ({ blocks: [paragraph(String(n))], selection: { anchor: pos(0, 0), focus: pos(0, 0) } });
    h.record(s(0), "type");
    t = 100;
    h.record(s(1), "type"); // grouped
    t = 200;
    h.seal();
    h.record(s(2), "type");
    const u1 = h.undo(s(3))!;
    expect(texts(u1.blocks)).toEqual(["2"]);
    const u2 = h.undo(u1)!;
    expect(texts(u2.blocks)).toEqual(["0"]);
    expect(h.canUndo).toBe(false);
    expect(texts(h.redo(u2)!.blocks)).toEqual(["2"]);
    h.record(s(9), "format");
    expect(h.canRedo).toBe(false);
  });
});
