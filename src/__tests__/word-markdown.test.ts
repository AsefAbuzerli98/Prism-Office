import { describe, expect, it } from "vitest";
import { blocksToHtml, htmlToBlocks, textToBlocks } from "../word/clipboard";
import { fromMarkdown, toMarkdown } from "../word/markdown";
import { BULLET_NUM_ID, DECIMAL_NUM_ID, paragraph, type Block, type Paragraph } from "../word/model";

const P = (b: Block) => b as Paragraph;

const sample: Block[] = [
  paragraph("Report", { style: "Heading1" }),
  {
    type: "paragraph",
    inlines: [
      { type: "text", text: "Plain ", marks: {} },
      { type: "text", text: "bold", marks: { bold: true } },
      { type: "text", text: " and ", marks: {} },
      { type: "text", text: "both", marks: { bold: true, italic: true } },
      { type: "text", text: " ", marks: {} },
      { type: "text", text: "code()", marks: { font: "Consolas" } },
      { type: "text", text: " ", marks: {} },
      { type: "text", text: "link", marks: { link: "https://example.com/a b" } },
      { type: "text", text: " 2*3 [x]", marks: {} },
    ],
  },
  paragraph("one", { list: { numId: BULLET_NUM_ID, level: 0 } }),
  paragraph("nested", { list: { numId: BULLET_NUM_ID, level: 1 } }),
  paragraph("first", { list: { numId: DECIMAL_NUM_ID, level: 0 } }),
  paragraph("A quote", { style: "Quote" }),
  paragraph("let x = 1;", { style: "Code" }),
  paragraph("x();", { style: "Code" }),
  paragraph(),
  { type: "opaque", label: "Table", xml: "<w:tbl/>" },
  { type: "paragraph", inlines: [{ type: "text", text: "see ", marks: {} }, { type: "opaque", label: "Image", xml: "<w:r><w:drawing/></w:r>" }] },
];

describe("markdown", () => {
  it("exports readable markdown", () => {
    const { markdown } = toMarkdown(sample);
    expect(markdown).toContain("# Report");
    expect(markdown).toContain("**bold**");
    expect(markdown).toContain("***both***");
    expect(markdown).toContain("`code()`");
    expect(markdown).toContain("[link](https://example.com/a%20b)");
    expect(markdown).toContain("2\\*3 \\[x\\]");
    expect(markdown).toContain("- one\n   - nested");
    expect(markdown).toContain("1. first");
    expect(markdown).toContain("> A quote");
    expect(markdown).toContain("```\nlet x = 1;\nx();\n```");
  });

  it("round-trips through markdown including preserved objects", () => {
    const { markdown, objects } = toMarkdown(sample);
    const back = fromMarkdown(markdown, objects);
    expect(back).toEqual(sample);
  });

  it("imports plain markdown", () => {
    const blocks = fromMarkdown("## Sub\n\nText with *em* and ~~gone~~<u>under</u>\\\nnext\n\n1. a\n2. b\n");
    expect(P(blocks[0]!).style).toBe("Heading2");
    const p = P(blocks[1]!);
    expect(p.inlines).toEqual([
      { type: "text", text: "Text with ", marks: {} },
      { type: "text", text: "em", marks: { italic: true } },
      { type: "text", text: " and ", marks: {} },
      { type: "text", text: "gone", marks: { strike: true } },
      { type: "text", text: "under", marks: { underline: true } },
      { type: "break", kind: "line" },
      { type: "text", text: "next", marks: {} },
    ]);
    expect(blocks.slice(2).map((b) => P(b).list)).toEqual([
      { numId: DECIMAL_NUM_ID, level: 0 },
      { numId: DECIMAL_NUM_ID, level: 0 },
    ]);
    expect(fromMarkdown("")).toHaveLength(1);
  });
});

describe("clipboard html", () => {
  it("round-trips formatting through HTML", () => {
    const frag: Block[] = [
      paragraph("Head", { style: "Heading2" }),
      { type: "paragraph", align: "center", inlines: [{ type: "text", text: "b", marks: { bold: true } }, { type: "text", text: " u", marks: { underline: true } }] },
      paragraph("i1", { list: { numId: DECIMAL_NUM_ID, level: 0 } }),
    ];
    const html = blocksToHtml(frag);
    expect(html).toContain("<h2>Head</h2>");
    expect(html).toContain("<ol><li>i1</li></ol>");
    expect(htmlToBlocks(html)).toEqual(frag);
  });

  it("parses foreign HTML (styles, links, nested lists, br)", () => {
    const blocks = htmlToBlocks(`<meta charset="utf-8"><div><span style="font-weight:700">Bold</span> <a href="https://x.y">x</a><br>line</div><ul><li>a<ul><li>b</li></ul></li></ul>`);
    const p = P(blocks[0]!);
    expect(p.inlines[0]).toEqual({ type: "text", text: "Bold", marks: { bold: true } });
    expect(p.inlines).toContainEqual({ type: "text", text: "x", marks: { link: "https://x.y" } });
    expect(p.inlines).toContainEqual({ type: "break", kind: "line" });
    expect(blocks.slice(1).map((b) => P(b).list)).toEqual([
      { numId: BULLET_NUM_ID, level: 0 },
      { numId: BULLET_NUM_ID, level: 1 },
    ]);
  });

  it("plain text paste splits lines and tabs", () => {
    const b = textToBlocks("a\tb\r\nc");
    expect(b).toHaveLength(2);
    expect(P(b[0]!).inlines.map((i) => i.type)).toEqual(["text", "tab", "text"]);
  });
});
