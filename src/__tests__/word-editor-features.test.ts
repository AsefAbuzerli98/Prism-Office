import { beforeEach, describe, expect, it } from "vitest";
import { BrowserHost } from "../core/host";
import { WordEditor } from "../word/editor";
import { monoMeasurer } from "../word/layout";
import { blockLabel, type Paragraph, type Table } from "../word/model";
import { paragraphText } from "../word/ops";
import { blocksToRtf, rtfToBlocks } from "../word/rtf";

const PNG_10x20 = btoa(String.fromCharCode(0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52, 0, 0, 0, 10, 0, 0, 0, 20, 8, 6, 0, 0, 0));

async function setup() {
  const host = new BrowserHost(localStorage, () => null);
  const info = await host.newDocument("document");
  const ed = new WordEditor(host, info.id, { measurer: monoMeasurer, syncDelayMs: 5, author: "Ana Test" });
  document.body.append(ed.el);
  await ed.ready;
  return { host, ed, id: info.id };
}
const input = (ed: WordEditor) => ed.el.querySelector(".word-input") as HTMLTextAreaElement;
const key = (ed: WordEditor, k: string, mods: KeyboardEventInit = {}) => input(ed).dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...mods }));
const type = (ed: WordEditor, text: string) => {
  input(ed).value = text;
  input(ed).dispatchEvent(new Event("input"));
};
const texts = (blocks: { type: string }[]) => blocks.map((b) => (b.type === "paragraph" ? paragraphText(b as Paragraph).replace(/￼/g, "") : `[${blockLabel(b as never)}]`));

describe("WordEditor features", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) => setTimeout(cb, 0)) as never;
  });

  it("inserts a table, types in cells, tabs between them and adds rows", async () => {
    const { ed } = await setup();
    type(ed, "Before");
    ed.execute("insertTable", "2x2");
    expect(ed.container).toEqual({ type: "cell", table: 1, row: 0, col: 0 });
    type(ed, "A1");
    key(ed, "Tab");
    type(ed, "B1");
    key(ed, "Tab");
    key(ed, "Tab");
    key(ed, "Tab"); // past last cell: new row
    type(ed, "A3");
    const t = ed.doc!.blocks[1] as Table;
    expect(t.rows).toHaveLength(3);
    expect(texts(t.rows[0]!.cells[1]!.blocks)).toEqual(["B1"]);
    expect(texts(t.rows[2]!.cells[0]!.blocks)).toEqual(["A3"]);
    ed.execute("rowAbove");
    ed.execute("colRight");
    expect((ed.doc!.blocks[1] as Table).rows.map((r) => r.cells.length)).toEqual([3, 3, 3, 3]);
    ed.execute("cellShading", "#ffcc00");
    expect((ed.doc!.blocks[1] as Table).rows[2]!.cells[1]!.shading).toBe("FFCC00");
    key(ed, "Escape");
    expect(ed.container).toBeNull();
    ed.execute("undo");
    expect((ed.doc!.blocks[1] as Table).rows[2]!.cells[1]!.shading).toBeUndefined();
  });

  it("footnotes: insert reference, edit note text, prune on delete", async () => {
    const { ed } = await setup();
    type(ed, "Claim");
    ed.execute("footnote");
    expect(ed.container).toEqual({ type: "note", kind: "footnote", id: 1 });
    type(ed, "Source text");
    expect(texts(ed.doc!.notes![0]!.blocks)).toEqual(["Source text"]);
    key(ed, "Escape");
    expect(ed.container).toBeNull();
    expect(ed.selection.focus).toEqual({ block: 0, offset: 6 });
    key(ed, "Backspace");
    expect(ed.doc!.notes).toEqual([]);
  });

  it("creates and edits a header", async () => {
    const { ed } = await setup();
    ed.execute("header");
    expect(ed.container).toEqual({ type: "header", index: 0 });
    type(ed, "Company");
    expect(ed.doc!.headers![0]).toMatchObject({ kind: "header", which: "default", relId: "" });
    expect(texts(ed.doc!.headers![0]!.blocks)).toEqual(["Company"]);
    const headerLines = ed.layout!.pages[0]!.lines.filter((l) => l.container?.type === "header");
    expect(headerLines.length).toBe(1);
  });

  it("tracked changes record insertions/deletions and can be accepted or rejected", async () => {
    const { ed } = await setup();
    type(ed, "base");
    ed.execute("trackChanges");
    expect(ed.state().tracking).toBe(true);
    type(ed, " new");
    const p = () => ed.doc!.blocks[0] as Paragraph;
    expect(p().inlines.at(-1)).toMatchObject({ text: " new", marks: { revision: { kind: "insert", author: "Ana Test" } } });
    key(ed, "Backspace"); // own insertion: really removed
    expect(paragraphText(p())).toBe("base ne");
    key(ed, "Home");
    key(ed, "Delete"); // original text: marked deleted, caret moves past it
    expect(paragraphText(p())).toBe("base ne");
    expect(p().inlines[0]).toMatchObject({ text: "b", marks: { revision: { kind: "delete" } } });
    ed.execute("rejectAll");
    expect(paragraphText(p())).toBe("base");
    expect(p().inlines.every((i) => i.type !== "text" || !i.marks.revision)).toBe(true);
  });

  it("comments anchor to the selection and show in the side panel", async () => {
    const { ed } = await setup();
    type(ed, "review this");
    ed.setSelection({ anchor: { block: 0, offset: 7 }, focus: { block: 0, offset: 11 } });
    ed.execute("comment", "Is this right?");
    expect(ed.doc!.comments).toHaveLength(1);
    expect(ed.doc!.comments![0]).toMatchObject({ author: "Ana Test", initials: "AT" });
    expect((ed.doc!.blocks[0] as Paragraph).inlines.filter((i) => i.type === "comment").map((i) => (i as { edge: string }).edge)).toEqual(["start", "end", "reference"]);
    expect(ed.el.querySelector(".comment-card textarea")?.textContent).toBe("Is this right?");
    ed.selectComment("0");
    expect(ed.selection).toEqual({ anchor: { block: 0, offset: 8 }, focus: { block: 0, offset: 12 } });
    ed.deleteComment("0");
    expect(ed.doc!.comments).toEqual([]);
    expect(paragraphText(ed.doc!.blocks[0] as Paragraph)).toBe("review this");
  });

  it("inserts, scales and describes pictures", async () => {
    const { ed } = await setup();
    await ed.insertImage(PNG_10x20);
    const img = (ed.doc!.blocks[0] as Paragraph).inlines[0];
    expect(img).toMatchObject({ type: "image", cx: 95250, cy: 190500 });
    ed.setSelection({ anchor: { block: 0, offset: 0 }, focus: { block: 0, offset: 1 } });
    expect(ed.state().object?.type).toBe("image");
    ed.execute("imageScale", "2");
    ed.execute("altText", "A chart");
    expect((ed.doc!.blocks[0] as Paragraph).inlines[0]).toMatchObject({ cx: 190500, descr: "A chart" });
    const frag = ed.layout!.pages[0]!.lines[0]!.frags.find((f) => f.kind === "image")!;
    expect(frag.width).toBeCloseTo(20);
  });

  it("table of contents and citations update from the document", async () => {
    const { ed } = await setup();
    type(ed, "Intro");
    ed.execute("style", "Heading1");
    key(ed, "End");
    key(ed, "Enter");
    type(ed, "Text ");
    ed.saveSource({ tag: "Doe24", kind: "Report", title: "Docs", authors: ["Doe, Jane"], year: "2024", publisher: "", url: "" });
    ed.execute("citation", "Doe24");
    expect(paragraphText(ed.doc!.blocks[1] as Paragraph).replace(/￼/g, "")).toBe("Text (Doe, 2024)");
    ed.setSelection({ anchor: { block: 0, offset: 0 }, focus: { block: 0, offset: 0 } });
    ed.execute("toc");
    expect(texts(ed.doc!.blocks).slice(0, 3)).toEqual(["Contents", "Intro\t1", "Intro"]);
    ed.execute("bibliography");
    expect(texts(ed.doc!.blocks)).toContain("Doe, J. (2024). Docs.");
  });
});

describe("RTF", () => {
  it("parses common clipboard RTF", () => {
    const rtf = String.raw`{\rtf1\ansi\deff0{\fonttbl{\f0 Calibri;}}{\colortbl;\red255\green0\blue0;}\pard\qc {\b Bold} and {\i it}\cf1 red\cf0\par
\pard caf\'e9 ` + "\\u601?x" + String.raw`\tab y\line z\par}`;
    const blocks = rtfToBlocks(rtf);
    expect(texts(blocks)).toEqual(["Bold and itred", "café əx\ty\nz"]);
    const p = blocks[0] as Paragraph;
    expect(p.align).toBe("center");
    expect(p.inlines[0]).toEqual({ type: "text", text: "Bold", marks: { bold: true } });
    expect(p.inlines).toContainEqual({ type: "text", text: "red", marks: { color: "FF0000" } });
  });

  it("round-trips through blocksToRtf", () => {
    const blocks = rtfToBlocks(blocksToRtf([{ type: "paragraph", align: "right", inlines: [{ type: "text", text: "Salam ", marks: {} }, { type: "text", text: "dünya", marks: { bold: true, color: "00AA00", size: 28 } }] }]));
    expect(blocks).toEqual([{ type: "paragraph", align: "right", inlines: [{ type: "text", text: "Salam ", marks: { size: 22 } }, { type: "text", text: "dünya", marks: { bold: true, color: "00AA00", size: 28 } }] }]);
  });
});
