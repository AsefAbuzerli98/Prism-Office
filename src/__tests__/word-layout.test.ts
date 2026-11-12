import { describe, expect, it } from "vitest";
import { breakParagraph, formatNumber, layoutDocument, listMarkers, monoMeasurer, tokenize } from "../word/layout";
import { blankDocument, BULLET_NUM_ID, DECIMAL_NUM_ID, paragraph, type WordDocument } from "../word/model";

const doc = (blocks: WordDocument["blocks"]): WordDocument => ({ ...blankDocument(), blocks, defaultPara: {}, defaultRun: { size: 20 } });
// size 20 half-points = 10pt = 13.333px → char width 6.667px with monoMeasurer.
const CW = (10 * 96) / 72 / 2;

describe("tokenize", () => {
  it("breaks after spaces, hyphens and around CJK", () => {
    expect(tokenize("hello  big-world end")).toEqual(["hello  ", "big-", "world ", "end"]);
    expect(tokenize("ab漢字cd")).toEqual(["ab", "漢", "字", "cd"]);
    expect(tokenize("-x")).toEqual(["-x"]);
  });
});

describe("breakParagraph", () => {
  it("wraps greedily with trailing spaces hanging", () => {
    const d = doc([paragraph("aaaa bbbb cccc")]);
    // 9 chars fit: "aaaa bbbb" (trailing space hangs) then "cccc"
    const lines = breakParagraph(d, d.blocks[0] as never, monoMeasurer, CW * 9.5, 0);
    expect(lines.map((l) => [l.start, l.end])).toEqual([[0, 10], [10, 14]]);
  });

  it("emergency-breaks words longer than the line", () => {
    const d = doc([paragraph("abcdefghij")]);
    const lines = breakParagraph(d, d.blocks[0] as never, monoMeasurer, CW * 4, 0);
    expect(lines.map((l) => l.frags.map((f) => f.text).join(""))).toEqual(["abcd", "efgh", "ij"]);
  });

  it("does not break inside a word spanning two runs", () => {
    const p = { ...paragraph(), inlines: [{ type: "text" as const, text: "xx hel", marks: {} }, { type: "text" as const, text: "lo", marks: { bold: true } }] };
    const d = doc([p]);
    const lines = breakParagraph(d, p, monoMeasurer, CW * 6, 0);
    expect(lines.map((l) => l.frags.map((f) => f.text).join(""))).toEqual(["xx ", "hello"]);
  });

  it("handles tabs and forced line breaks", () => {
    const p = { ...paragraph(), inlines: [{ type: "text" as const, text: "a", marks: {} }, { type: "tab" as const, marks: {} }, { type: "text" as const, text: "b", marks: {} }, { type: "break" as const, kind: "line" as const }, { type: "text" as const, text: "c", marks: {} }] };
    const d = doc([p]);
    const lines = breakParagraph(d, p, monoMeasurer, 500, 0);
    expect(lines).toHaveLength(2);
    const tab = lines[0]!.frags.find((f) => f.kind === "tab")!;
    expect(tab.x + tab.width).toBeCloseTo(48);
    expect(lines[1]!.frags.map((f) => f.text).join("")).toBe("c");
  });
});

describe("layoutDocument", () => {
  it("aligns and justifies lines", () => {
    const text = "word ".repeat(40).trim();
    const d = doc([paragraph("mid", { align: "center" }), paragraph(text, { align: "justify" })]);
    const L = layoutDocument(d, monoMeasurer);
    const mid = L.pages[0]!.lines[0]!;
    const contentW = L.width - L.margin.left - L.margin.right;
    expect(mid.frags[0]!.x - L.margin.left).toBeCloseTo((contentW - 3 * CW) / 2, 1);
    const just = L.pages[0]!.lines[1]!;
    const lastFrag = just.frags[just.frags.length - 1]!;
    const trailing = /\s$/.test(lastFrag.text) ? CW : 0;
    expect(lastFrag.x + lastFrag.width - trailing).toBeCloseTo(L.width - L.margin.right, 1);
  });

  it("paginates and honours explicit page breaks and pageBreakBefore", () => {
    const many = Array.from({ length: 80 }, (_, i) => paragraph(`line ${i}`));
    const L = layoutDocument(doc(many), monoMeasurer);
    expect(L.pages.length).toBeGreaterThan(1);
    for (const p of L.pages) for (const l of p.lines) expect(l.y + l.height).toBeLessThanOrEqual(L.height - L.margin.bottom + 0.01);

    const withBreak = doc([
      { ...paragraph("a"), inlines: [{ type: "text", text: "a", marks: {} }, { type: "break", kind: "page" }, { type: "text", text: "b", marks: {} }] },
      paragraph("c", { pageBreakBefore: true }),
    ]);
    const L2 = layoutDocument(withBreak, monoMeasurer);
    expect(L2.pages.map((p) => p.lines.map((l) => l.frags.map((f) => f.text).join("")))).toEqual([["a"], ["b"], ["c"]]);
  });

  it("keeps a heading with the next paragraph", () => {
    const fill = Array.from({ length: 200 }, () => paragraph("x"));
    const L0 = layoutDocument(doc(fill), monoMeasurer);
    const perPage = L0.pages[0]!.lines.length;
    const blocks = [...fill.slice(0, perPage - 1), paragraph("Heading", { style: "Heading1" }), paragraph("body")];
    const L = layoutDocument(doc(blocks), monoMeasurer);
    const headingPage = L.pages.findIndex((p) => p.lines.some((l) => l.block === perPage - 1));
    const bodyPage = L.pages.findIndex((p) => p.lines.some((l) => l.block === perPage));
    expect(headingPage).toBe(bodyPage);
    expect(headingPage).toBe(1);
  });

  it("renders opaque blocks as boxes and list markers", () => {
    const d = doc([
      paragraph("one", { list: { numId: DECIMAL_NUM_ID, level: 0 } }),
      paragraph("sub", { list: { numId: DECIMAL_NUM_ID, level: 1 } }),
      paragraph("two", { list: { numId: DECIMAL_NUM_ID, level: 0 } }),
      paragraph("dot", { list: { numId: BULLET_NUM_ID, level: 0 } }),
      { type: "opaque", label: "Table", xml: "<w:tbl/>" },
    ]);
    expect([...listMarkers(d).values()]).toEqual(["1.", "a.", "2.", "•"]);
    const L = layoutDocument(d, monoMeasurer);
    expect(L.pages[0]!.boxes[0]!.label).toBe("Table");
    const first = L.pages[0]!.lines[0]!;
    expect(first.frags[0]!.kind).toBe("marker");
    expect(first.frags[1]!.x).toBeGreaterThan(first.frags[0]!.x);
  });

  it("formats list numbers", () => {
    expect([formatNumber(4, "upperRoman"), formatNumber(28, "lowerLetter"), formatNumber(3, "decimal")]).toEqual(["IV", "ab", "3"]);
  });

  it("reuses cached paragraph breaks for unchanged paragraphs", () => {
    const p = paragraph("cached text");
    const d = doc([p]);
    const a = layoutDocument(d, monoMeasurer);
    const b = layoutDocument(d, monoMeasurer);
    expect(a.pages[0]!.lines[0]!.frags[0]).not.toBe(b.pages[0]!.lines[0]!.frags[0]); // cloned, not shared
    expect(a.pages[0]!.lines[0]!.frags[0]!.x).toBe(b.pages[0]!.lines[0]!.frags[0]!.x);
  });
});
