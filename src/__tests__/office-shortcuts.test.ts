import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserHost, type HostBridge } from "../core/host";
import { formatKeys, shortcutFor, shortcutSections } from "../core/shortcuts";
import { iconNames, iconSvg } from "../core/icons";
import { RIBBONS } from "../core/ribbons";
import { commandSize } from "../shell/ribbon";
import { SheetEditor, dateSerial } from "../sheet/editor";
import { loadEngineSync, Workbook } from "../sheet/engine";
import { WordEditor } from "../word/editor";
import { monoMeasurer } from "../word/layout";
import type { Paragraph } from "../word/model";
import { paragraphText } from "../word/ops";

beforeAll(() => {
  loadEngineSync(readFileSync(resolve(__dirname, "../sheet/wasm/prism_sheet_bg.wasm")));
  HTMLCanvasElement.prototype.getContext = (() => null) as never;
});

async function word() {
  const host = new BrowserHost(localStorage, () => null);
  const info = await host.newDocument("document");
  const ed = new WordEditor(host, info.id, { measurer: monoMeasurer, syncDelayMs: 5 });
  document.body.append(ed.el);
  await ed.ready;
  return ed;
}
const input = (ed: WordEditor) => ed.el.querySelector(".word-input") as HTMLTextAreaElement;
const key = (ed: WordEditor, k: string, mods: KeyboardEventInit = {}) =>
  input(ed).dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ctrlKey: true, ...mods }));
const type = (ed: WordEditor, text: string) => {
  input(ed).value = text;
  input(ed).dispatchEvent(new Event("input"));
};
const para = (ed: WordEditor, i = 0) => ed.doc!.blocks[i] as Paragraph;
const selectRange = (ed: WordEditor, a: number, b: number, block = 0) => ed.setSelection({ anchor: { block, offset: a }, focus: { block, offset: b } });

describe("Word shortcuts", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) => setTimeout(cb, 0)) as never;
  });

  it("grows and shrinks the font in Word's steps and by one point", async () => {
    const ed = await word();
    type(ed, "Hello");
    selectRange(ed, 0, 5);
    key(ed, ">", { shiftKey: true, code: "Period" });
    expect(ed.state().size).toBe(12);
    key(ed, "]");
    expect(ed.state().size).toBe(13);
    key(ed, "<", { shiftKey: true, code: "Comma" });
    expect(ed.state().size).toBe(12);
  });

  it("toggles subscript and superscript, and Ctrl+Space clears character formatting", async () => {
    const ed = await word();
    type(ed, "H2O");
    selectRange(ed, 1, 2);
    key(ed, "=", { code: "Equal" });
    expect(ed.state().vertAlign).toBe("subscript");
    key(ed, "+", { shiftKey: true, code: "Equal" });
    expect(ed.state().vertAlign).toBe("superscript");
    key(ed, "b");
    expect(ed.state().bold).toBe(true);
    key(ed, " ");
    expect(ed.state().bold).toBe(false);
    expect(ed.state().vertAlign).toBeNull();
  });

  it("Shift+F3 cycles lower, UPPER and Capitalised case", async () => {
    const ed = await word();
    type(ed, "hello big world");
    selectRange(ed, 0, 15);
    key(ed, "F3", { ctrlKey: false, shiftKey: true });
    expect(paragraphText(para(ed))).toBe("HELLO BIG WORLD");
    key(ed, "F3", { ctrlKey: false, shiftKey: true });
    expect(paragraphText(para(ed))).toBe("Hello Big World");
    key(ed, "F3", { ctrlKey: false, shiftKey: true });
    expect(paragraphText(para(ed))).toBe("hello big world");
  });

  it("headings, line spacing, indent and copy/paste formatting", async () => {
    const ed = await word();
    type(ed, "Title");
    key(ed, "2", { altKey: true, code: "Digit2" });
    expect(para(ed).style).toBe("Heading2");
    key(ed, "n", { shiftKey: true });
    expect(para(ed).style).toBeUndefined();
    key(ed, "5");
    expect(para(ed).line).toBe(360);
    key(ed, "m");
    expect(para(ed).indentLeft).toBeGreaterThan(0);
    selectRange(ed, 0, 5);
    key(ed, "i");
    key(ed, "c", { shiftKey: true });
    key(ed, "i"); // clear italic again
    expect(ed.state().italic).toBe(false);
    key(ed, "v", { shiftKey: true });
    expect(ed.state().italic).toBe(true);
  });

  it("finds, replaces and replaces all (one undo step)", async () => {
    const ed = await word();
    type(ed, "north and North and south");
    expect(ed.findAll("north")).toHaveLength(2);
    expect(ed.findAll("north", true)).toHaveLength(1);
    ed.setSelection({ anchor: { block: 0, offset: 0 }, focus: { block: 0, offset: 0 } });
    expect(ed.findNext("north")).toBe(0);
    expect(ed.findNext("north")).toBe(1);
    expect(ed.findNext("north")).toBe(0); // wraps
    ed.replaceNext("north", "west");
    expect(paragraphText(para(ed))).toBe("west and North and south");
    expect(ed.replaceAll("and", "&")).toBe(2);
    expect(paragraphText(para(ed))).toBe("west & North & south");
    ed.undo();
    expect(paragraphText(para(ed))).toBe("west and North and south");
  });

  it("Ctrl+H opens the find bar with the replace row", async () => {
    const ed = await word();
    type(ed, "abc abc");
    key(ed, "h");
    const bar = ed.el.querySelector<HTMLElement>(".find-bar")!;
    expect(bar.hidden).toBe(false);
    expect(bar.classList.contains("with-replace")).toBe(true);
    const q = bar.querySelector<HTMLInputElement>(".find-query")!;
    q.value = "abc";
    q.dispatchEvent(new Event("input"));
    expect(bar.querySelector(".find-count")!.textContent).toMatch(/2/);
  });
});

describe("Excel shortcuts", () => {
  function sheet() {
    const host = { updateSheetModel: vi.fn(async () => {}), getSheetModel: vi.fn() } as unknown as HostBridge;
    const ed = new SheetEditor(host, 1, { workbook: new Workbook() });
    document.body.append(ed.el);
    return ed;
  }
  const k = (ed: SheetEditor, key: string, extra: KeyboardEventInit = {}) => ed.onKey(new KeyboardEvent("keydown", { key, cancelable: true, ...extra }));

  it("number formats, strikethrough, select column/row, last cell, sheet switching", async () => {
    const ed = sheet();
    await ed.ready;
    ed.applyEdits([{ sheet: 0, r: 0, c: 0, text: "0.25" }, { sheet: 0, r: 4, c: 3, text: "x" }]);
    ed.selectCell(0, 0);
    k(ed, "%", { ctrlKey: true, shiftKey: true, code: "Digit5" });
    expect(ed.state().numFormat).toBe("0%");
    k(ed, "$", { ctrlKey: true, shiftKey: true, code: "Digit4" });
    expect(ed.state().numFormat).toBe("$#,##0.00");
    k(ed, "5", { ctrlKey: true, code: "Digit5" });
    expect(ed.state().style.strike).toBe(true);
    k(ed, " ", { ctrlKey: true });
    expect(ed.selection).toMatchObject({ r0: 0, c0: 0, c1: 0 });
    expect(ed.selection.r1).toBeGreaterThan(1000);
    k(ed, " ", { shiftKey: true });
    expect(ed.selection.c1).toBeGreaterThan(1000);
    ed.selectCell(0, 0);
    k(ed, "End", { ctrlKey: true });
    expect(ed.active).toEqual({ r: 4, c: 3 });
    ed.execute("addSheet");
    expect(ed.sheet).toBe(1);
    k(ed, "PageUp", { ctrlKey: true });
    expect(ed.sheet).toBe(0);
  });

  it("Ctrl+; enters today's date as a formatted serial", async () => {
    const ed = sheet();
    await ed.ready;
    ed.selectCell(2, 2);
    k(ed, ";", { ctrlKey: true });
    expect(ed.wb!.input(0, 2, 2)).toBe(String(dateSerial()));
    expect(ed.state().numFormat).toBe("yyyy-mm-dd");
    expect(dateSerial(new Date(2024, 0, 15))).toBe(45306);
  });

  it("Ctrl+Enter fills the selection with the entry, adjusting references", async () => {
    const ed = sheet();
    await ed.ready;
    ed.applyEdits([{ sheet: 0, r: 0, c: 0, text: "1" }, { sheet: 0, r: 1, c: 0, text: "2" }]);
    ed.selectCell(0, 1);
    k(ed, "ArrowDown", { shiftKey: true });
    ed.beginEdit("=A1*10");
    const ta = ed.el.querySelector<HTMLTextAreaElement>(".sheet-cell-editor")!;
    ta.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true, cancelable: true }));
    expect(ed.wb!.input(0, 1, 1)).toBe("=A2*10");
  });
});

describe("shortcut catalogue and ribbon", () => {
  it("lists each mode's shortcuts and formats them for tooltips", () => {
    for (const m of ["document", "workbook", "presentation"] as const) expect(shortcutSections(m).length).toBeGreaterThan(2);
    expect(shortcutFor("doc.bold", false)).toBe("Ctrl+B");
    expect(shortcutFor("doc.bold", true)).toBe("⌘B");
    expect(shortcutFor("ppt.showCurrent", false)).toBe("Shift+F5");
    expect(formatKeys("Mod+ArrowUp", false)).toBe("Ctrl+↑");
  });

  it("every drawn icon renders and ribbon commands get a size", () => {
    for (const n of iconNames()) expect(iconSvg(n)).toMatch(/^<svg[^>]+class="ic ic-\w+"/);
    expect(commandSize("doc.bold")).toBe("mini");
    expect(commandSize("xl.copy")).toBe("row");
    expect(commandSize("xl.paste")).toBe("large");
    for (const tabs of Object.values(RIBBONS)) {
      const ids = tabs.flatMap((t) => t.groups.flatMap((g) => g.commands.map((c) => c.id)));
      expect(new Set(ids).size).toBe(ids.length);
    }
  });
});
