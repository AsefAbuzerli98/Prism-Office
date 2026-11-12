import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, it, vi } from "vitest";
import type { HostBridge } from "../core/host";
import { SheetEditor, parseTsv } from "../sheet/editor";
import { loadEngineSync, Workbook } from "../sheet/engine";
import { Axis, parseRange, tokenAt } from "../sheet/geometry";

beforeAll(() => {
  loadEngineSync(readFileSync(resolve(__dirname, "../sheet/wasm/prism_sheet_bg.wasm")));
  // jsdom has no canvas; drawing is exercised in the browser screenshot check
  HTMLCanvasElement.prototype.getContext = (() => null) as never;
});

function editor() {
  const updates: unknown[] = [];
  const host = { updateSheetModel: vi.fn(async (_id: number, m: unknown) => void updates.push(m)), getSheetModel: vi.fn() } as unknown as HostBridge;
  const dirty = vi.fn();
  const errors: string[] = [];
  const ed = new SheetEditor(host, 1, { workbook: new Workbook(), onDirty: dirty, onError: (m) => errors.push(m) });
  document.body.append(ed.el);
  return { ed, host, updates, dirty, errors };
}

const key = (ed: SheetEditor, k: string, extra: Partial<KeyboardEventInit> = {}) => ed.onKey(new KeyboardEvent("keydown", { key: k, cancelable: true, ...extra }));
const type = (ed: SheetEditor, text: string, commit = "Enter") => {
  key(ed, text[0]!);
  const ta = ed.el.querySelector<HTMLTextAreaElement>(".sheet-cell-editor")!;
  ta.value = text;
  ta.dispatchEvent(new Event("input"));
  ta.dispatchEvent(new KeyboardEvent("keydown", { key: commit, cancelable: true }));
};
const shown = (ed: SheetEditor, r: number, c: number) => ed.wb!.view(ed.sheet, { r0: r, c0: c, r1: r, c1: c })[0]?.t ?? "";

describe("geometry", () => {
  it("maps offsets with custom sizes", () => {
    const a = new Axis(20, 100, [[2, 50], [5, 10]]);
    expect(a.offset(3)).toBe(90);
    expect(a.offset(6)).toBe(140);
    expect(a.at(89)).toBe(2);
    expect(a.at(90)).toBe(3);
    expect(a.at(125)).toBe(4);
    expect(a.at(135)).toBe(5);
    expect(parseRange("b2:a1")).toEqual({ r0: 0, c0: 0, r1: 1, c1: 1 });
    expect(tokenAt("=SUM(XLO", 8)).toEqual({ start: 5, word: "XLO" });
    expect(tokenAt('="XLO', 5)).toBeNull();
    expect(tokenAt("plain", 3)).toBeNull();
    expect(parseTsv('a\t"b\tc"\n1\t2\n')).toEqual([["a", "b\tc"], ["1", "2"]]);
  });
});

describe("SheetEditor", () => {
  it("types values and formulas, moves like Excel and recalculates", async () => {
    const { ed, dirty } = editor();
    await ed.ready;
    type(ed, "10");
    expect(ed.active).toEqual({ r: 1, c: 0 });
    type(ed, "32", "Tab");
    expect(ed.active).toEqual({ r: 1, c: 1 });
    type(ed, "x");
    expect(ed.active).toEqual({ r: 2, c: 0 });
    ed.selectCell(1, 1);
    key(ed, "Delete");
    ed.selectCell(2, 0);
    type(ed, "=SUM(A1:A2)");
    expect(shown(ed, 2, 0)).toBe("42");
    expect(dirty).toHaveBeenCalled();
    ed.selectCell(0, 0);
    type(ed, "8");
    expect(shown(ed, 2, 0)).toBe("40");
    // undo / redo
    ed.execute("undo");
    expect(shown(ed, 2, 0)).toBe("42");
    ed.execute("redo");
    expect(shown(ed, 2, 0)).toBe("40");
    // formula bar shows the input text
    ed.selectCell(2, 0);
    expect(ed.el.querySelector<HTMLInputElement>(".sheet-formula")!.value).toBe("=SUM(A1:A2)");
    // Delete clears the selection
    key(ed, "Delete");
    expect(shown(ed, 2, 0)).toBe("");
  });

  it("keeps the editor open on a bad formula and reports it", async () => {
    const { ed, errors } = editor();
    await ed.ready;
    type(ed, "=SUM(");
    expect(errors.length).toBe(1);
    expect(ed.isEditing).toBe(true);
    ed.cancelEdit();
    expect(ed.isEditing).toBe(false);
  });

  it("autocompletes function names", async () => {
    const { ed } = editor();
    await ed.ready;
    key(ed, "=");
    const ta = ed.el.querySelector<HTMLTextAreaElement>(".sheet-cell-editor")!;
    ta.value = "=XLOO";
    ta.setSelectionRange(5, 5);
    ta.dispatchEvent(new Event("input"));
    const ac = ed.el.querySelector<HTMLUListElement>(".sheet-ac")!;
    expect(ac.hidden).toBe(false);
    expect([...ac.querySelectorAll("li")].map((l) => l.textContent)).toEqual(["XLOOKUP"]);
    ta.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", cancelable: true }));
    expect(ta.value).toBe("=XLOOKUP(");
    expect(ac.hidden).toBe(true);
  });

  it("fills series, copies with shifted formulas and styles cells", async () => {
    const { ed } = editor();
    await ed.ready;
    type(ed, "1");
    type(ed, "2");
    // series via engine fill (drag handle path)
    ed.wb!.fill(0, { r0: 0, c0: 0, r1: 1, c1: 0 }, { r0: 0, c0: 0, r1: 4, c1: 0 });
    expect(shown(ed, 4, 0)).toBe("5");
    // Ctrl+D copies the top row down
    ed.anchor = { r: 0, c: 2 };
    ed.extent = { r: 2, c: 2 };
    ed.wb!.set(0, 0, 2, "x");
    ed.execute("fillDown");
    expect(shown(ed, 2, 2)).toBe("x");
    ed.selectCell(0, 1);
    type(ed, "=A1*10");
    ed.selectCell(0, 1);
    ed.execute("copy");
    ed.selectCell(3, 1);
    ed.execute("paste");
    expect(ed.wb!.input(0, 3, 1)).toBe("=A4*10");
    expect(shown(ed, 3, 1)).toBe("40");
    ed.selectCell(3, 1);
    ed.execute("bold");
    ed.execute("numfmt", "0.00");
    expect(ed.state().style.bold).toBe(true);
    expect(shown(ed, 3, 1)).toBe("40.00");
    ed.execute("fill", "#FFF2CC");
    expect(ed.state().style.fill).toBe("#FFF2CC");
    // external TSV paste
    ed.selectCell(10, 0);
    ed.pasteText("a\tb\n3\t=A12*2");
    expect(shown(ed, 11, 1)).toBe("6");
    expect(ed.selectionText()).toBe("a");
  });

  it("manages sheets and pushes the model to the host", async () => {
    vi.useFakeTimers();
    const { ed, updates } = editor();
    await ed.ready;
    type(ed, "7");
    ed.execute("addSheet");
    expect(ed.sheet).toBe(1);
    expect([...ed.el.querySelectorAll(".sheet-tab:not(.add)")].map((b) => b.textContent)).toEqual(["Sheet1", "Sheet2"]);
    type(ed, "=Sheet1!A1+1");
    expect(shown(ed, 0, 0)).toBe("8");
    ed.switchSheet(0);
    expect(ed.sheet).toBe(0);
    ed.selectCell(1, 1);
    ed.execute("freeze");
    expect(ed.state().frozen).toBe(true);
    vi.advanceTimersByTime(700);
    expect(updates.length).toBeGreaterThan(0);
    await ed.flush();
    const m = updates.at(-1) as { sheets: { name: string }[] };
    expect(m.sheets.map((s) => s.name)).toEqual(["Sheet1", "Sheet2"]);
    vi.useRealTimers();
  });

  it("navigates with Ctrl+arrows across data blocks", async () => {
    const { ed } = editor();
    await ed.ready;
    for (let i = 0; i < 5; i++) type(ed, String(i));
    ed.selectCell(0, 0);
    key(ed, "ArrowDown", { ctrlKey: true });
    expect(ed.active.r).toBe(4);
    key(ed, "ArrowDown", { ctrlKey: true, shiftKey: true });
    expect(ed.selection.r1).toBe(1_048_575);
  });

  it("supplies =PY results asynchronously", async () => {
    const onAsync = vi.fn();
    const host = { updateSheetModel: vi.fn(async () => {}) } as unknown as HostBridge;
    const ed = new SheetEditor(host, 1, { workbook: new Workbook(), onAsync });
    await ed.ready;
    ed.applyEdits([{ sheet: 0, r: 0, c: 0, text: '=PY("1+1",0)' }]);
    expect(onAsync).toHaveBeenCalled();
    const [, reqs] = onAsync.mock.calls.at(-1)!;
    ed.supplyAsync(reqs[0], 2);
    expect(shown(ed, 0, 0)).toBe("2");
  });
});

describe("charts, pivots and data import", () => {
  it("parses CSV/JSON and builds pivot formulas", async () => {
    const { jsonToRows, parseCsv, pivotFormula, sniffDelimiter } = await import("../sheet/data");
    expect(sniffDelimiter("a;b;c\n1;2;3")).toBe(";");
    expect(parseCsv('name,note\r\n"Doe, J","said ""hi""\nthere"\n')).toEqual([["name", "note"], ["Doe, J", 'said "hi"\nthere']]);
    expect(jsonToRows([{ a: 1 }, { a: 2, b: "x" }])).toEqual([["a", "b"], [1, null], [2, "x"]]);
    expect(jsonToRows({ data: [[1, 2]] })).toEqual([[1, 2]]);
    const data = { r0: 0, c0: 0, r1: 4, c1: 2 };
    expect(pivotFormula({ sheet: "Sales Data", data, rows: 0, cols: 1, values: 2, fn: "SUM", qualify: true })).toBe("=PIVOTBY('Sales Data'!A2:A5,'Sales Data'!B2:B5,'Sales Data'!C2:C5,SUM)");
    expect(pivotFormula({ sheet: "S", data, rows: 0, cols: null, values: 2, fn: "AVERAGE", qualify: false })).toBe("=GROUPBY(A2:A5,C2:C5,AVERAGE)");
  });

  it("imports rows safely, inserts charts and refreshes connections", async () => {
    const { ed } = editor();
    await ed.ready;
    const sheet = ed.importRows("sales.csv", [["Region", "Sales"], ["East", "10"], ["West", "=HYPERLINK(\"x\")"], ["North", "30"]], { kind: "csv", source: "/data/sales.csv" });
    expect(ed.sheet).toBe(sheet);
    expect(ed.wb!.input(sheet, 2, 1)).toBe("'=HYPERLINK(\"x\")"); // CSV formula injection stays text
    expect(ed.connections[0]?.sheet).toBe("sales.csv");
    ed.selectCell(0, 0);
    const i = ed.insertChart("column", "Sales")!;
    const charts = ed.wb!.charts(sheet);
    expect(charts[i]!.chart.anchor.c0).toBe(3);
    expect(charts[i]!.data.categories).toEqual(["East", "West", "North"]);
    expect(charts[i]!.data.series[0]).toEqual({ name: "Sales", values: [10, null, 30] });
    expect(ed.replaceRows("sales.csv", [["Region", "Sales"], ["South", "5"]])).toBe(true);
    expect(ed.wb!.input(sheet, 2, 0)).toBe("");
    expect(ed.wb!.charts(sheet)[0]!.data.series[0]!.values[0]).toBe(5);
    ed.selectedChart = 0;
    ed.onKey(new KeyboardEvent("keydown", { key: "Delete", cancelable: true }));
    expect(ed.wb!.charts(sheet)).toEqual([]);
  });
});

describe("classic PivotTables", () => {
  it("adds, refreshes and edits charts in place", async () => {
    const { ed } = editor();
    await ed.ready;
    ed.importRows("data", [["Region", "Sales"], ["East", "10"], ["West", "20"], ["East", "5"]]);
    const s = ed.sheet;
    expect(ed.addPivot(s, { name: "", sourceSheet: "data", source: { r0: 0, c0: 0, r1: 3, c1: 1 }, rows: 0, values: 1, function: "sum", location: { row: 0, col: 3 } })).toBeNull();
    expect(shown(ed, 1, 3)).toBe("East");
    expect(shown(ed, 1, 4)).toBe("15");
    expect(shown(ed, 3, 4)).toBe("35");
    ed.wb!.set(s, 1, 1, "100");
    ed.execute("refreshPivots");
    expect(shown(ed, 1, 4)).toBe("105");
    // a pivot may not overwrite data
    expect(ed.addPivot(s, { name: "", sourceSheet: "data", source: { r0: 0, c0: 0, r1: 3, c1: 1 }, rows: 0, values: 1, function: "sum", location: { row: 0, col: 0 } })).toMatch(/already data/);
    ed.selectCell(0, 0);
    ed.insertChart("column");
    ed.execute("chart", "pie"); // a selected chart changes type
    expect(ed.wb!.charts(s)[0]!.chart.kind).toBe("pie");
  });
});

describe("=PY pictures", () => {
  it("shows a matplotlib figure in the cell", async () => {
    const onAsync = vi.fn();
    const host = { updateSheetModel: vi.fn(async () => {}) } as unknown as HostBridge;
    const ed = new SheetEditor(host, 1, { workbook: new Workbook(), onAsync });
    await ed.ready;
    ed.applyEdits([{ sheet: 0, r: 2, c: 1, text: '=PY("import matplotlib.pyplot as plt\nplt.plot([1,2])",0)' }]);
    const [, reqs] = onAsync.mock.calls.at(-1)!;
    ed.supplyAsync(reqs[0], { image: "iVBORw0KGgo=", width: 400, height: 300 });
    expect(shown(ed, 2, 1)).toBe("Image");
    expect(ed.imageAt(0, 2, 1)).toEqual({ w: 400, h: 300 });
    expect(ed.wb!.sheets()[0]!.rowHeights.length).toBe(1);
  });
});
