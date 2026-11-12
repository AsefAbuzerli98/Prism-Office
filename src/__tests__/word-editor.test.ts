import { beforeEach, describe, expect, it } from "vitest";
import { BrowserHost } from "../core/host";
import { monoMeasurer } from "../word/layout";
import { WordEditor } from "../word/editor";
import { paragraphText } from "../word/ops";
import { blockLabel } from "../word/model";
import type { Paragraph } from "../word/model";

const texts = (ed: WordEditor) => ed.doc!.blocks.map((b) => (b.type === "paragraph" ? paragraphText(b) : `[${blockLabel(b)}]`));

async function setup() {
  const host = new BrowserHost(localStorage, () => null);
  const info = await host.newDocument("document");
  let dirty = 0;
  const ed = new WordEditor(host, info.id, { measurer: monoMeasurer, onDirty: () => dirty++, syncDelayMs: 5 });
  document.body.append(ed.el);
  await ed.ready;
  return { host, ed, id: info.id, dirty: () => dirty };
}

const key = (ed: WordEditor, k: string, mods: KeyboardEventInit = {}) =>
  (ed.el.querySelector(".word-input") as HTMLTextAreaElement).dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...mods }));

const type = (ed: WordEditor, text: string) => {
  const input = ed.el.querySelector(".word-input") as HTMLTextAreaElement;
  input.value = text;
  input.dispatchEvent(new Event("input"));
};

describe("WordEditor", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) => setTimeout(cb, 0)) as never;
  });

  it("types, splits paragraphs, deletes and undoes", async () => {
    const { ed, dirty } = await setup();
    type(ed, "Hello");
    key(ed, "Enter");
    type(ed, "World");
    expect(texts(ed)).toEqual(["Hello", "World"]);
    expect(dirty()).toBe(1);
    key(ed, "Backspace");
    expect(texts(ed)).toEqual(["Hello", "Worl"]);
    key(ed, "ArrowLeft", { altKey: true });
    key(ed, "Backspace");
    expect(texts(ed)).toEqual(["HelloWorl"]);
    key(ed, "z", { ctrlKey: true });
    expect(texts(ed)).toEqual(["Hello", "Worl"]);
    key(ed, "z", { ctrlKey: true, shiftKey: true });
    expect(texts(ed)).toEqual(["HelloWorl"]);
  });

  it("toggles marks on a selection and for upcoming typing", async () => {
    const { ed } = await setup();
    type(ed, "abc");
    key(ed, "ArrowLeft", { shiftKey: true });
    key(ed, "ArrowLeft", { shiftKey: true });
    key(ed, "b", { ctrlKey: true });
    const p = ed.doc!.blocks[0] as Paragraph;
    expect(p.inlines).toEqual([
      { type: "text", text: "a", marks: {} },
      { type: "text", text: "bc", marks: { bold: true } },
    ]);
    expect(ed.state().bold).toBe(true);
    key(ed, "End");
    key(ed, "i", { ctrlKey: true });
    type(ed, "d");
    expect((ed.doc!.blocks[0] as Paragraph).inlines.at(-1)).toEqual({ type: "text", text: "d", marks: { bold: true, italic: true } });
  });

  it("lists: toggle, tab indents, enter on empty item exits", async () => {
    const { ed } = await setup();
    type(ed, "item");
    ed.execute("bullets");
    expect((ed.doc!.blocks[0] as Paragraph).list).toEqual({ numId: 9001, level: 0 });
    key(ed, "Tab");
    expect((ed.doc!.blocks[0] as Paragraph).list?.level).toBe(1);
    key(ed, "Enter");
    expect((ed.doc!.blocks[1] as Paragraph).list?.level).toBe(1);
    key(ed, "Enter");
    expect((ed.doc!.blocks[1] as Paragraph).list).toBeUndefined();
  });

  it("headings continue as Normal after Enter at the end", async () => {
    const { ed } = await setup();
    type(ed, "Title");
    ed.execute("style", "Heading1");
    key(ed, "Enter");
    expect((ed.doc!.blocks[1] as Paragraph).style).toBeUndefined();
    expect(ed.state().style).toBe("Normal");
  });

  it("maps clicks to positions and positions to caret rects consistently", async () => {
    const { ed } = await setup();
    type(ed, "hello world");
    const r = ed.caretRect({ block: 0, offset: 6 })!;
    expect(ed.hitTest(r.x + 1, r.y + r.h / 2)).toEqual({ block: 0, offset: 6 });
    const end = ed.caretRect({ block: 0, offset: 11 })!;
    expect(ed.hitTest(end.x + 300, end.y + 2)).toEqual({ block: 0, offset: 11 });
  });

  it("syncs edits to the host and flushes before save", async () => {
    const { ed, host, id } = await setup();
    type(ed, "persist me");
    await ed.flush();
    const stored = await host.getWordDocument(id);
    expect(paragraphText(stored.blocks[0] as Paragraph)).toBe("persist me");
  });

  it("markdown pane reflects and drives the document", async () => {
    const { ed } = await setup();
    type(ed, "Intro");
    ed.execute("markdown");
    const md = ed.el.querySelector(".word-md") as HTMLTextAreaElement;
    expect(md.hidden).toBe(false);
    expect(md.value).toBe("Intro\n");
    ed.applyMarkdown("# Big\n\n- a\n- b\n");
    expect(texts(ed)).toEqual(["Big", "a", "b"]);
    expect((ed.doc!.blocks[0] as Paragraph).style).toBe("Heading1");
    key(ed, "z", { ctrlKey: true });
    expect(texts(ed)).toEqual(["Intro"]);
  });

  it("copies and pastes with formatting via the clipboard events", async () => {
    const { ed } = await setup();
    type(ed, "copy me");
    ed.selectAll();
    key(ed, "b", { ctrlKey: true });
    const data = new Map<string, string>();
    const dt = { setData: (k: string, v: string) => data.set(k, v), getData: (k: string) => data.get(k) ?? "" };
    const input = ed.el.querySelector(".word-input")!;
    const cut = new Event("cut", { cancelable: true }) as ClipboardEvent;
    Object.defineProperty(cut, "clipboardData", { value: dt });
    input.dispatchEvent(cut);
    expect(texts(ed)).toEqual([""]);
    expect(data.get("text/html")).toContain("<strong>copy me</strong>");
    const paste = new Event("paste", { cancelable: true }) as ClipboardEvent;
    Object.defineProperty(paste, "clipboardData", { value: dt });
    input.dispatchEvent(paste);
    input.dispatchEvent(paste);
    expect(texts(ed)).toEqual(["copy mecopy me"]);
    expect((ed.doc!.blocks[0] as Paragraph).inlines[0]).toMatchObject({ marks: { bold: true } });
  });

  it("page setup commands change the section", async () => {
    const { ed } = await setup();
    ed.execute("orientation", "landscape");
    expect(ed.doc!.section.landscape).toBe(true);
    expect(ed.doc!.section.pageWidth).toBe(16838);
    ed.execute("margins", "narrow");
    expect(ed.doc!.section.marginLeft).toBe(720);
  });
});
