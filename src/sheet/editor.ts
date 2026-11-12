/**
 * Workbook mode: virtualized canvas grid over the WASM engine with a
 * formula bar, in-cell editor, function autocomplete, fill handle,
 * frozen panes, column/row resizing, clipboard, undo and sheet tabs.
 */
import { h } from "../core/dom";
import type { HostBridge } from "../core/host";
import { MODES } from "../core/modes";
import type { EngineView } from "../word/editor";
import { drawChart } from "./charts";
import { loadEngine, Workbook, type AsyncRequest, type Change, type ChartData, type SheetInfo, type ViewCell } from "./engine";
import { a1, Axis, colChars, colName, colPx, DEFAULT_COL_PX, DEFAULT_ROW_PX, MAX_COLS, MAX_ROWS, parseRange, rowPt, rowPx, tokenAt } from "./geometry";
import type { CellStyle, ChartDef, ChartKind, PivotDef, Rect } from "./model";

const HDR_W_MIN = 46;
const HDR_H = 22;
const MAX_SCROLL_PX = 10_000_000;
const ACCENT = MODES.workbook.accent;

export interface DataConnection {
  kind: "csv" | "json" | "sqlite" | "odbc";
  /** File path or connection string. */
  source: string;
  query?: string | null;
  /** Sheet the data lands on. */
  sheet: string;
}

export interface SheetEditorOptions {
  onDirty?: () => void;
  onState?: () => void;
  onError?: (message: string) => void;
  /** `=PY` cells are waiting for the Python runtime. */
  onAsync?: (editor: SheetEditor, requests: AsyncRequest[]) => void;
  /** Supply a preloaded engine (tests). */
  workbook?: Workbook;
}

interface Edit {
  sheet: number;
  r: number;
  c: number;
  text: string;
}

const norm = (a: { r: number; c: number }, b: { r: number; c: number }): Rect => ({
  r0: Math.min(a.r, b.r),
  c0: Math.min(a.c, b.c),
  r1: Math.max(a.r, b.r),
  c1: Math.max(a.c, b.c),
});

const key = (r: number, c: number) => r * MAX_COLS + c;

export class SheetEditor implements EngineView {
  readonly el: HTMLElement;
  readonly ready: Promise<void>;
  wb: Workbook | null = null;
  sheet = 0;
  /** Active cell and the anchor/extent of the selection. */
  active = { r: 0, c: 0 };
  anchor = { r: 0, c: 0 };
  extent = { r: 0, c: 0 };

  private info: SheetInfo[] = [];
  private styles: CellStyle[] = [];
  private cols = new Axis(DEFAULT_COL_PX, MAX_COLS);
  private rows = new Axis(DEFAULT_ROW_PX, MAX_ROWS);
  private vRows = 200;
  private vCols = 40;
  private scroll: HTMLDivElement;
  private spacer: HTMLDivElement;
  private canvas: HTMLCanvasElement;
  private main: HTMLDivElement;
  private cellEditor: HTMLTextAreaElement;
  private formula: HTMLInputElement;
  private nameBox: HTMLInputElement;
  private tabsBar: HTMLDivElement;
  private stats: HTMLSpanElement;
  private ac: HTMLUListElement;
  private acItems: string[] = [];
  private acIndex = 0;
  private editing: { r: number; c: number; source: "cell" | "bar" } | null = null;
  private drag: null | {
    kind: "select" | "fill" | "colResize" | "rowResize" | "colSelect" | "rowSelect" | "ref" | "chartMove" | "chartResize";
    index?: number;
    start?: number;
    size?: number;
    refStart?: { r: number; c: number };
    refPos?: number;
    origin?: { r: number; c: number };
    anchor?: Rect;
  } = null;
  /** Charts of the current sheet (refreshed on every draw). */
  private charts: { chart: ChartDef; data: ChartData }[] = [];
  selectedChart: number | null = null;
  /** Anchor of the chart being dragged (drawn instead of the stored one). */
  private chartDrag: { index: number; anchor: Rect } | null = null;
  private fillTarget: Rect | null = null;
  private undoStack: Edit[][] = [];
  private redoStack: Edit[][] = [];
  private clip: { sheet: number; rect: Rect; cut: boolean } | null = null;
  private pushTimer: ReturnType<typeof setTimeout> | null = null;
  private drawQueued = false;
  private disposed = false;
  private resizeObs: ResizeObserver | null = null;
  private functions: string[] = [];
  /** Column where a Tab-entry run started (Enter returns there, like Excel). */
  private tabStart: number | null = null;

  constructor(
    private host: HostBridge,
    private docId: number,
    private opts: SheetEditorOptions = {},
  ) {
    this.canvas = h("canvas", { class: "sheet-canvas", "aria-hidden": "true" });
    this.spacer = h("div", { class: "sheet-spacer" });
    this.scroll = h(
      "div",
      { class: "sheet-scroll", tabindex: "0", role: "grid", "aria-label": "Spreadsheet grid", "aria-multiselectable": "true" },
      this.spacer,
    );
    this.cellEditor = h("textarea", { class: "sheet-cell-editor", spellcheck: "false", "aria-label": "Cell editor", hidden: true });
    this.ac = h("ul", { class: "sheet-ac", role: "listbox", hidden: true });
    this.main = h("div", { class: "sheet-main" }, this.scroll, this.canvas, this.cellEditor, this.ac);
    this.nameBox = h("input", { class: "sheet-namebox", "aria-label": "Name box", spellcheck: "false" });
    this.formula = h("input", { class: "sheet-formula", "aria-label": "Formula bar", spellcheck: "false" });
    this.tabsBar = h("div", { class: "sheet-tabs", role: "tablist" });
    this.stats = h("span", { class: "sheet-stats", "aria-live": "polite" });
    this.el = h(
      "div",
      { class: "sheet-editor" },
      h("div", { class: "sheet-fbar" }, this.nameBox, h("span", { class: "sheet-fx" }, "fx"), this.formula),
      this.main,
      h("div", { class: "sheet-bottom" }, this.tabsBar, this.stats),
    );
    this.wire();
    this.ready = this.load();
  }

  // ---- lifecycle ---------------------------------------------------------------------------

  private async load() {
    if (this.opts.workbook) {
      this.wb = this.opts.workbook;
    } else {
      const [model] = await Promise.all([this.host.getSheetModel(this.docId), loadEngine()]);
      this.wb = new Workbook(model);
    }
    this.functions = Workbook.functionNames();
    this.refreshInfo();
    this.selectCell(0, 0);
    this.checkAsync();
  }

  async flush(): Promise<void> {
    if (this.editing) this.commit(0, 0);
    if (this.pushTimer) {
      clearTimeout(this.pushTimer);
      this.pushTimer = null;
    }
    if (this.wb) await this.host.updateSheetModel(this.docId, this.wb.model());
  }

  focus() {
    this.scroll.focus({ preventScroll: true });
  }

  dispose() {
    this.disposed = true;
    this.resizeObs?.disconnect();
    if (this.pushTimer) clearTimeout(this.pushTimer);
    this.wb?.dispose();
    this.wb = null;
  }

  private changed(ch: Change | null) {
    if (!ch) return;
    if (ch.error) this.opts.onError?.(ch.error);
    this.opts.onDirty?.();
    if (this.pushTimer) clearTimeout(this.pushTimer);
    this.pushTimer = setTimeout(() => {
      this.pushTimer = null;
      if (this.wb && !this.disposed) void this.host.updateSheetModel(this.docId, this.wb.model());
    }, 600);
    if (ch.asyncRequests > 0) this.checkAsync();
    this.refreshInfo();
  }

  private checkAsync() {
    const reqs = this.wb?.asyncRequests() ?? [];
    if (reqs.length) this.opts.onAsync?.(this, reqs);
  }

  /** Pictures returned by =PY cells (matplotlib figures), by "sheet:r:c". */
  private images = new Map<string, { img: HTMLImageElement; w: number; h: number }>();

  /** Answer an async (=PY) request; `{image: base64}` shows a picture in the cell. */
  supplyAsync(req: AsyncRequest, value: unknown) {
    if (!this.wb) return;
    const k = `${req.sheet}:${req.row}:${req.col}`;
    const pic = value && typeof value === "object" && "image" in value ? (value as { image: string; width?: number; height?: number }) : null;
    if (pic) {
      const img = new Image();
      img.onload = () => this.requestDraw();
      img.src = `data:image/png;base64,${pic.image}`;
      const w = pic.width ?? 400;
      const h = pic.height ?? 300;
      this.images.set(k, { img, w, h });
      // make the first picture readable: grow a small cell to a thumbnail
      const info = this.wb.sheets()[req.sheet];
      const hasW = info?.colWidths.some(([c]) => c === req.col);
      const hasH = info?.rowHeights.some(([r]) => r === req.row);
      if (!hasW) this.wb.setColWidth(req.sheet, req.col, colChars(Math.min(320, w * 0.6)));
      if (!hasH) this.wb.setRowHeight(req.sheet, req.row, rowPt(Math.min(240, h * 0.6)));
      value = "Image";
    } else this.images.delete(k);
    this.changed(this.wb.supplyAsync(req, value));
  }

  /** Picture shown in a cell, if any (tests / export). */
  imageAt(sheet: number, r: number, c: number): { w: number; h: number } | null {
    const p = this.images.get(`${sheet}:${r}:${c}`);
    return p ? { w: p.w, h: p.h } : null;
  }

  private refreshInfo() {
    if (!this.wb) return;
    this.info = this.wb.sheets();
    this.styles = this.wb.styles();
    this.charts = this.wb.charts(Math.min(this.sheet, this.info.length - 1));
    this.sheet = Math.min(this.sheet, this.info.length - 1);
    const s = this.info[this.sheet]!;
    this.cols.set(s.colWidths.map(([c, w]) => [c, colPx(w)]));
    this.rows.set(s.rowHeights.map(([r, ht]) => [r, rowPx(ht)]));
    const used = s.used;
    this.vRows = Math.min(MAX_ROWS, Math.max(this.vRows, (used?.r1 ?? 0) + 200, this.active.r + 100));
    this.vCols = Math.min(MAX_COLS, Math.max(this.vCols, (used?.c1 ?? 0) + 20, this.active.c + 20));
    this.spacer.style.width = `${this.hdrW + this.cols.offset(this.vCols)}px`;
    this.syncSpacerH();
    this.renderTabs();
    this.requestDraw();
    this.opts.onState?.();
  }

  // ---- geometry ----------------------------------------------------------------------------------

  private get frozen(): [number, number] {
    return this.info[this.sheet]?.frozen ?? [0, 0];
  }

  /** Row-header width: wide enough for the largest row number in use. */
  private get hdrW(): number {
    const digits = String(Math.max(this.vRows, this.active.r + 1)).length;
    return Math.max(HDR_W_MIN, Math.ceil(12 + digits * 7.4));
  }

  /** Real content height of the virtual area. */
  private contentH(): number {
    return HDR_H + this.rows.offset(this.vRows);
  }

  /** The spacer is capped (browsers limit element height to ~17–33M px);
   * beyond that, scroll positions are scaled onto the real content. */
  private syncSpacerH() {
    this.spacer.style.height = `${Math.min(this.contentH(), MAX_SCROLL_PX)}px`;
  }

  /** Vertical content offset of the scrolled area (unscaled). */
  get scrollY(): number {
    const s = this.scroll;
    const real = this.contentH();
    if (real <= MAX_SCROLL_PX) return s.scrollTop;
    const maxS = MAX_SCROLL_PX - s.clientHeight;
    return maxS > 0 ? (s.scrollTop * (real - s.clientHeight)) / maxS : 0;
  }

  set scrollY(v: number) {
    const s = this.scroll;
    const real = this.contentH();
    if (real <= MAX_SCROLL_PX) {
      s.scrollTop = v;
      return;
    }
    const maxC = real - s.clientHeight;
    s.scrollTop = maxC > 0 ? (v * (MAX_SCROLL_PX - s.clientHeight)) / maxC : 0;
  }

  /** Screen x of column c (relative to the grid element). */
  colX(c: number): number {
    const fc = this.frozen[1];
    const x = this.hdrW + this.cols.offset(c);
    return c < fc ? x : x - this.scroll.scrollLeft;
  }

  rowY(r: number): number {
    const fr = this.frozen[0];
    const y = HDR_H + this.rows.offset(r);
    return r < fr ? y : y - this.scrollY;
  }

  /** Cell under a screen point (relative to the grid element). */
  cellAt(x: number, y: number): { r: number; c: number } {
    const [fr, fc] = this.frozen;
    const fw = this.cols.offset(fc);
    const fh = this.rows.offset(fr);
    const gx = x - this.hdrW < fw ? x - this.hdrW : x - this.hdrW + this.scroll.scrollLeft;
    const gy = y - HDR_H < fh ? y - HDR_H : y - HDR_H + this.scrollY;
    return { r: this.rows.at(Math.max(0, gy)), c: this.cols.at(Math.max(0, gx)) };
  }

  get selection(): Rect {
    return norm(this.anchor, this.extent);
  }

  // ---- events ------------------------------------------------------------------------------------

  private wire() {
    this.scroll.addEventListener("scroll", () => {
      // grow the virtual area as the user approaches its end
      const s = this.scroll;
      if (this.scrollY + s.clientHeight * 2 > HDR_H + this.rows.offset(this.vRows) && this.vRows < MAX_ROWS) {
        this.vRows = Math.min(MAX_ROWS, this.vRows * 2);
        this.syncSpacerH();
      }
      if (s.scrollLeft + s.clientWidth * 2 > this.hdrW + this.cols.offset(this.vCols) && this.vCols < MAX_COLS) {
        this.vCols = Math.min(MAX_COLS, this.vCols * 2);
        this.spacer.style.width = `${this.hdrW + this.cols.offset(this.vCols)}px`;
      }
      if (this.editing?.source === "cell") this.placeEditor();
      this.requestDraw();
    });
    this.scroll.addEventListener("mousedown", (e) => this.onMouseDown(e));
    this.scroll.addEventListener("dblclick", (e) => {
      const p = this.point(e);
      const hit = this.chartAt(p.x, p.y);
      if (hit >= 0) {
        const t = window.prompt("Chart title", this.charts[hit]?.chart.title ?? "");
        if (t !== null) {
          this.selectedChart = hit;
          this.editChart({ title: t });
        }
        return;
      }
      if (p.y > HDR_H && p.x > this.hdrW) this.beginEdit(null);
    });
    window.addEventListener("mousemove", (e) => this.onMouseMove(e));
    window.addEventListener("mouseup", () => this.onMouseUp());
    this.scroll.addEventListener("keydown", (e) => this.onKey(e));
    this.scroll.addEventListener("copy", (e) => this.onCopy(e, false));
    this.scroll.addEventListener("cut", (e) => this.onCopy(e, true));
    this.scroll.addEventListener("paste", (e) => this.onPaste(e));
    this.scroll.addEventListener("wheel", () => this.requestDraw(), { passive: true });

    const editorKeys = (e: KeyboardEvent, source: "cell" | "bar") => {
      if (!this.ac.hidden && this.acItems.length) {
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          this.acIndex = (this.acIndex + (e.key === "ArrowDown" ? 1 : this.acItems.length - 1)) % this.acItems.length;
          this.renderAc();
          return;
        }
        if (e.key === "Tab" || (e.key === "Enter" && !e.altKey)) {
          e.preventDefault();
          this.acceptAc(source === "cell" ? this.cellEditor : this.formula);
          return;
        }
        if (e.key === "Escape") {
          e.preventDefault();
          this.hideAc();
          return;
        }
      }
      if (e.key === "Enter" && e.altKey && source === "cell") {
        e.preventDefault();
        const t = this.cellEditor;
        const p = t.selectionStart;
        t.setRangeText("\n", p, t.selectionEnd, "end");
        this.syncEditors(source);
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        this.commit(e.shiftKey ? -1 : 1, 0);
      } else if (e.key === "Tab") {
        e.preventDefault();
        this.commit(0, e.shiftKey ? -1 : 1);
      } else if (e.key === "Escape") {
        e.preventDefault();
        this.cancelEdit();
      } else if (e.key === "F4") {
        e.preventDefault();
        this.toggleAbsolute(source === "cell" ? this.cellEditor : this.formula);
      }
    };
    this.cellEditor.addEventListener("keydown", (e) => editorKeys(e, "cell"));
    this.cellEditor.addEventListener("input", () => this.syncEditors("cell"));
    this.formula.addEventListener("keydown", (e) => editorKeys(e, "bar"));
    this.formula.addEventListener("focus", () => {
      if (!this.editing) this.editing = { ...this.active, source: "bar" };
    });
    this.formula.addEventListener("input", () => this.syncEditors("bar"));
    this.nameBox.addEventListener("keydown", (e) => {
      if (e.key !== "Enter") return;
      e.preventDefault();
      const r = parseRange(this.nameBox.value);
      if (r) {
        this.anchor = { r: r.r0, c: r.c0 };
        this.active = { ...this.anchor };
        this.extent = { r: r.r1, c: r.c1 };
        this.reveal(this.active);
        this.selectionChanged();
        this.focus();
      } else {
        this.nameBox.value = a1(this.active.r, this.active.c);
      }
    });
    if (typeof ResizeObserver !== "undefined") {
      this.resizeObs = new ResizeObserver(() => this.requestDraw());
      this.resizeObs.observe(this.main);
    }
  }

  private point(e: MouseEvent) {
    const b = this.main.getBoundingClientRect();
    return { x: e.clientX - b.left, y: e.clientY - b.top };
  }

  private onMouseDown(e: MouseEvent) {
    if (e.button !== 0 || !this.wb) return;
    const { x, y } = this.point(e);
    // clicks on the scrollbars
    if (x > this.scroll.clientWidth || y > this.scroll.clientHeight) return;
    const pointing = this.editing && this.isPointMode();
    if (!pointing) {
      if (this.editing) this.commit(0, 0);
      this.focus();
    }
    e.preventDefault();
    if (y < HDR_H && x < this.hdrW) {
      this.anchor = { r: 0, c: 0 };
      this.extent = { r: MAX_ROWS - 1, c: MAX_COLS - 1 };
      this.active = { r: 0, c: 0 };
      this.selectionChanged();
      return;
    }
    const cell = this.cellAt(x, y);
    if (y < HDR_H) {
      const edge = this.edgeNear(x, "col");
      if (edge !== null) {
        this.drag = { kind: "colResize", index: edge, start: e.clientX, size: this.cols.sizeOf(edge) };
        return;
      }
      if (e.shiftKey) this.extent = { r: MAX_ROWS - 1, c: cell.c };
      else {
        this.anchor = { r: 0, c: cell.c };
        this.active = { r: 0, c: cell.c };
        this.extent = { r: MAX_ROWS - 1, c: cell.c };
      }
      this.drag = { kind: "colSelect" };
      this.selectionChanged();
      return;
    }
    if (x < this.hdrW) {
      const edge = this.edgeNear(y, "row");
      if (edge !== null) {
        this.drag = { kind: "rowResize", index: edge, start: e.clientY, size: this.rows.sizeOf(edge) };
        return;
      }
      if (e.shiftKey) this.extent = { r: cell.r, c: MAX_COLS - 1 };
      else {
        this.anchor = { r: cell.r, c: 0 };
        this.active = { r: cell.r, c: 0 };
        this.extent = { r: cell.r, c: MAX_COLS - 1 };
      }
      this.drag = { kind: "rowSelect" };
      this.selectionChanged();
      return;
    }
    const hit = pointing ? -1 : this.chartAt(x, y);
    if (hit >= 0) {
      const ch = this.charts[hit]!.chart;
      this.selectedChart = hit;
      const corner = Math.abs(x - this.colX(ch.anchor.c1 + 1)) < 10 && Math.abs(y - this.rowY(ch.anchor.r1 + 1)) < 10;
      this.drag = { kind: corner ? "chartResize" : "chartMove", index: hit, origin: cell, anchor: { ...ch.anchor } };
      this.requestDraw();
      this.opts.onState?.();
      return;
    }
    if (this.selectedChart !== null) {
      this.selectedChart = null;
      this.opts.onState?.();
    }
    if (pointing) {
      const input = this.editing!.source === "cell" ? this.cellEditor : this.formula;
      const pos = input.selectionStart ?? input.value.length;
      const ref = this.refText(cell, cell);
      input.setRangeText(ref, pos, input.selectionEnd ?? pos, "end");
      this.drag = { kind: "ref", refStart: cell, refPos: pos };
      this.syncEditors(this.editing!.source);
      input.focus();
      return;
    }
    const sel = this.selection;
    const hx = this.colX(sel.c1 + 1);
    const hy = this.rowY(sel.r1 + 1);
    if (Math.abs(x - hx) <= 4 && Math.abs(y - hy) <= 4 && sel.r1 < MAX_ROWS - 1) {
      this.drag = { kind: "fill" };
      this.fillTarget = sel;
      return;
    }
    if (e.shiftKey) this.extent = cell;
    else {
      this.active = cell;
      this.anchor = cell;
      this.extent = cell;
    }
    this.drag = { kind: "select" };
    this.selectionChanged();
  }

  private edgeNear(v: number, axis: "col" | "row"): number | null {
    if (axis === "col") {
      const c = this.cellAt(v, HDR_H + 1).c;
      if (Math.abs(this.colX(c + 1) - v) <= 3) return c;
      if (c > 0 && Math.abs(this.colX(c) - v) <= 3) return c - 1;
    } else {
      const r = this.cellAt(this.hdrW + 1, v).r;
      if (Math.abs(this.rowY(r + 1) - v) <= 3) return r;
      if (r > 0 && Math.abs(this.rowY(r) - v) <= 3) return r - 1;
    }
    return null;
  }

  private onMouseMove(e: MouseEvent) {
    const d = this.drag;
    if (!d || !this.wb) return;
    const { x, y } = this.point(e);
    const cell = this.cellAt(Math.max(this.hdrW + 1, x), Math.max(HDR_H + 1, y));
    switch (d.kind) {
      case "select":
        this.extent = cell;
        this.autoScroll(x, y);
        this.selectionChanged();
        break;
      case "colSelect":
        this.extent = { r: MAX_ROWS - 1, c: cell.c };
        this.selectionChanged();
        break;
      case "rowSelect":
        this.extent = { r: cell.r, c: MAX_COLS - 1 };
        this.selectionChanged();
        break;
      case "fill": {
        const s = this.selection;
        const down = cell.r - s.r1;
        const right = cell.c - s.c1;
        this.fillTarget = down >= right ? { ...s, r1: Math.max(s.r1, cell.r) } : { ...s, c1: Math.max(s.c1, cell.c) };
        this.autoScroll(x, y);
        this.requestDraw();
        break;
      }
      case "colResize": {
        const px = Math.max(4, d.size! + e.clientX - d.start!);
        this.cols.set([...this.overrides("col").filter(([c]) => c !== d.index), [d.index!, px]]);
        this.requestDraw();
        break;
      }
      case "rowResize": {
        const px = Math.max(4, d.size! + e.clientY - d.start!);
        this.rows.set([...this.overrides("row").filter(([r]) => r !== d.index), [d.index!, px]]);
        this.requestDraw();
        break;
      }
      case "chartMove":
      case "chartResize": {
        const dr = cell.r - d.origin!.r;
        const dc = cell.c - d.origin!.c;
        const a = d.anchor!;
        const next: Rect =
          d.kind === "chartMove"
            ? { r0: Math.max(0, a.r0 + dr), c0: Math.max(0, a.c0 + dc), r1: Math.max(0, a.r0 + dr) + (a.r1 - a.r0), c1: Math.max(0, a.c0 + dc) + (a.c1 - a.c0) }
            : { ...a, r1: Math.max(a.r0 + 2, a.r1 + dr), c1: Math.max(a.c0 + 1, a.c1 + dc) };
        this.chartDrag = { index: d.index!, anchor: next };
        this.requestDraw();
        break;
      }
      case "ref": {
        const input = this.editing?.source === "bar" ? this.formula : this.cellEditor;
        const before = input.value.slice(0, d.refPos!);
        const old = /^[$A-Z0-9:]*/.exec(input.value.slice(d.refPos!))?.[0] ?? "";
        input.value = before + this.refText(d.refStart!, cell) + input.value.slice(d.refPos! + old.length);
        this.syncEditors(this.editing?.source ?? "cell");
        break;
      }
    }
  }

  private overrides(axis: "col" | "row"): [number, number][] {
    const s = this.info[this.sheet]!;
    return axis === "col" ? s.colWidths.map(([c, w]) => [c, colPx(w)]) : s.rowHeights.map(([r, ht]) => [r, rowPx(ht)]);
  }

  private onMouseUp() {
    const d = this.drag;
    this.drag = null;
    if (!d || !this.wb) return;
    if (d.kind === "fill" && this.fillTarget) {
      const src = this.selection;
      const dst = this.fillTarget;
      this.fillTarget = null;
      if (dst.r1 !== src.r1 || dst.c1 !== src.c1) {
        this.recordUndo(this.cellsOf(dst));
        this.changed(this.wb.fill(this.sheet, src, dst));
        this.extent = { r: dst.r1, c: dst.c1 };
        this.selectionChanged();
      } else this.requestDraw();
    } else if (d.kind === "chartMove" || d.kind === "chartResize") {
      const ch = this.charts[d.index!]?.chart;
      const moved = this.chartDrag;
      this.chartDrag = null;
      if (ch && moved && JSON.stringify(moved.anchor) !== JSON.stringify(d.anchor)) {
        this.wb.updateChart(this.sheet, d.index!, { ...ch, anchor: moved.anchor, fromFile: false });
        this.changed({ changed: [], evaluated: 0, circular: [], asyncRequests: 0 });
      } else this.requestDraw();
    } else if (d.kind === "colResize") {
      this.wb.setColWidth(this.sheet, d.index!, colChars(this.cols.sizeOf(d.index!)));
      this.changed({ changed: [], evaluated: 0, circular: [], asyncRequests: 0 });
    } else if (d.kind === "rowResize") {
      this.wb.setRowHeight(this.sheet, d.index!, rowPt(this.rows.sizeOf(d.index!)));
      this.changed({ changed: [], evaluated: 0, circular: [], asyncRequests: 0 });
    }
  }

  private autoScroll(x: number, y: number) {
    const s = this.scroll;
    if (y > s.clientHeight - 10) this.scrollY += 20;
    if (y < HDR_H + 4 && this.scrollY > 0) this.scrollY -= 20;
    if (x > s.clientWidth - 10) s.scrollLeft += 30;
    if (x < this.hdrW + 4 && s.scrollLeft > 0) s.scrollLeft -= 30;
  }

  /** Keyboard handling while the grid (not an editor) has focus. Public for tests. */
  onKey(e: KeyboardEvent) {
    if (!this.wb || this.editing) return;
    if (this.selectedChart !== null) {
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        this.wb.removeChart(this.sheet, this.selectedChart);
        this.selectedChart = null;
        this.changed({ changed: [], evaluated: 0, circular: [], asyncRequests: 0 });
        return;
      }
      if (e.key === "Escape") {
        this.selectedChart = null;
        this.requestDraw();
        return;
      }
      this.selectedChart = null;
    }
    const mod = e.ctrlKey || e.metaKey;
    const move = (dr: number, dc: number) => {
      e.preventDefault();
      const from = e.shiftKey ? this.extent : this.active;
      let { r, c } = from;
      if (mod) ({ r, c } = this.jump(from, dr, dc));
      else {
        r = Math.min(MAX_ROWS - 1, Math.max(0, r + dr));
        c = Math.min(MAX_COLS - 1, Math.max(0, c + dc));
      }
      if (e.shiftKey) this.extent = { r, c };
      else {
        this.active = { r, c };
        this.anchor = { r, c };
        this.extent = { r, c };
      }
      this.reveal({ r, c });
      this.selectionChanged();
    };
    const page = Math.max(1, Math.floor(this.scroll.clientHeight / DEFAULT_ROW_PX) - 1);
    switch (e.key) {
      case "ArrowUp":
        return move(-1, 0);
      case "ArrowDown":
        return move(1, 0);
      case "ArrowLeft":
        return move(0, -1);
      case "ArrowRight":
        return move(0, 1);
      case "PageDown":
        return move(page, 0);
      case "PageUp":
        return move(-page, 0);
      case "Home":
        return move(mod ? -this.active.r : 0, -(e.shiftKey ? this.extent.c : this.active.c));
      case "Enter":
        return this.moveWithin(e.shiftKey ? -1 : 1, 0, e);
      case "Tab":
        return this.moveWithin(0, e.shiftKey ? -1 : 1, e);
      case "F2":
        e.preventDefault();
        return this.beginEdit(null);
      case "Delete":
      case "Backspace":
        e.preventDefault();
        return this.clearSelection();
    }
    if (mod) {
      const k = e.key.toLowerCase();
      const run = (cmd: string) => {
        e.preventDefault();
        this.execute(cmd);
      };
      if (k === "z") return run(e.shiftKey ? "redo" : "undo");
      if (k === "y") return run("redo");
      if (k === "b") return run("bold");
      if (k === "i") return run("italic");
      if (k === "u") return run("underline");
      if (k === "a") return run("selectAll");
      if (k === "d") return run("fillDown");
      if (k === "r") return run("fillRight");
      return;
    }
    if (e.key === "=" && e.altKey) {
      e.preventDefault();
      return void this.execute("autoSum");
    }
    if (e.key.length === 1 && !e.altKey) {
      e.preventDefault();
      this.beginEdit(e.key);
    }
  }

  /** Enter/Tab move inside a multi-cell selection like Excel. */
  private moveWithin(dr: number, dc: number, e: Event) {
    e.preventDefault();
    const s = this.selection;
    if (s.r0 === s.r1 && s.c0 === s.c1) {
      const r = Math.min(MAX_ROWS - 1, Math.max(0, this.active.r + dr));
      const c = Math.min(MAX_COLS - 1, Math.max(0, this.active.c + dc));
      this.selectCell(r, c);
      return;
    }
    let { r, c } = this.active;
    r += dr;
    c += dc;
    if (c > s.c1) {
      c = s.c0;
      r++;
    } else if (c < s.c0) {
      c = s.c1;
      r--;
    }
    if (r > s.r1) {
      r = s.r0;
      c = c + (dr ? 1 : 0) > s.c1 ? s.c0 : c + (dr ? 1 : 0);
    } else if (r < s.r0) r = s.r1;
    this.active = { r, c };
    this.reveal(this.active);
    this.selectionChanged();
  }

  /** Ctrl+arrow: jump to the edge of the current data block. */
  private jump(from: { r: number; c: number }, dr: number, dc: number) {
    const filled = (r: number, c: number) => this.wb!.view(this.sheet, { r0: r, c0: c, r1: r, c1: c }).some((v) => v.k !== "-");
    let { r, c } = from;
    const step = () => {
      r = Math.min(MAX_ROWS - 1, Math.max(0, r + dr));
      c = Math.min(MAX_COLS - 1, Math.max(0, c + dc));
    };
    const used = this.info[this.sheet]?.used;
    const limitR = dr > 0 ? Math.max(used?.r1 ?? 0, r) + 1 : 0;
    const limitC = dc > 0 ? Math.max(used?.c1 ?? 0, c) + 1 : 0;
    const startFilled = filled(r, c);
    step();
    if (startFilled && filled(r, c)) {
      while (true) {
        const pr = r;
        const pc = c;
        step();
        if ((pr === r && pc === c) || !filled(r, c)) {
          r = pr;
          c = pc;
          break;
        }
      }
    } else {
      let guard = 0;
      while (!filled(r, c) && guard++ < 1_100_000) {
        if ((dr > 0 && r >= limitR) || (dc > 0 && c >= limitC)) {
          r = dr > 0 ? MAX_ROWS - 1 : r;
          c = dc > 0 ? MAX_COLS - 1 : c;
          break;
        }
        const pr = r;
        const pc = c;
        step();
        if (pr === r && pc === c) break;
      }
    }
    return { r, c };
  }

  private reveal(p: { r: number; c: number }) {
    const s = this.scroll;
    const [fr, fc] = this.frozen;
    if (p.r >= this.vRows - 10) {
      this.vRows = Math.min(MAX_ROWS, p.r + 200);
      this.syncSpacerH();
    }
    if (p.c >= this.vCols - 5) {
      this.vCols = Math.min(MAX_COLS, p.c + 20);
      this.spacer.style.width = `${this.hdrW + this.cols.offset(this.vCols)}px`;
    }
    if (p.r >= fr) {
      const top = this.rows.offset(p.r) - this.rows.offset(fr);
      const bottom = this.rows.offset(p.r + 1);
      const viewH = s.clientHeight - HDR_H;
      if (top < this.scrollY) this.scrollY = top;
      else if (viewH > 0 && bottom - this.scrollY > viewH) this.scrollY = bottom - viewH;
    }
    if (p.c >= fc) {
      const left = this.cols.offset(p.c) - this.cols.offset(fc);
      const right = this.cols.offset(p.c + 1);
      const viewW = s.clientWidth - this.hdrW;
      if (left < s.scrollLeft) s.scrollLeft = left;
      else if (viewW > 0 && right - s.scrollLeft > viewW) s.scrollLeft = right - viewW;
    }
  }

  selectCell(r: number, c: number) {
    this.active = { r, c };
    this.anchor = { r, c };
    this.extent = { r, c };
    this.reveal(this.active);
    this.selectionChanged();
  }

  private selectionChanged() {
    if (!this.wb) return;
    const s = this.selection;
    this.nameBox.value = s.r0 === s.r1 && s.c0 === s.c1 ? a1(s.r0, s.c0) : `${a1(s.r0, s.c0)}:${a1(s.r1, s.c1)}`;
    if (!this.editing) this.formula.value = this.wb.input(this.sheet, this.active.r, this.active.c);
    this.updateStats();
    this.scroll.setAttribute("aria-activedescendant", "");
    this.scroll.setAttribute("aria-label", `Spreadsheet grid, ${a1(this.active.r, this.active.c)}: ${this.formula.value || "empty"}`);
    this.requestDraw();
    this.opts.onState?.();
  }

  private updateStats() {
    const s = this.selection;
    const used = this.info[this.sheet]?.used;
    if (!used || (s.r0 === s.r1 && s.c0 === s.c1)) {
      this.stats.textContent = "";
      return;
    }
    const clip = { r0: s.r0, c0: s.c0, r1: Math.min(s.r1, used.r1), c1: Math.min(s.c1, used.c1) };
    if (clip.r1 < clip.r0 || clip.c1 < clip.c0 || (clip.r1 - clip.r0 + 1) * (clip.c1 - clip.c0 + 1) > 500_000) {
      this.stats.textContent = "";
      return;
    }
    const cells = this.wb!.view(this.sheet, clip).filter((v) => v.k !== "-" && v.k !== "f");
    const nums = cells.filter((v) => v.k === "n").map((v) => v.n ?? 0);
    const sum = nums.reduce((a, b) => a + b, 0);
    this.stats.textContent = nums.length
      ? `Average: ${fmt(sum / nums.length)}   Count: ${cells.length}   Sum: ${fmt(sum)}`
      : `Count: ${cells.length}`;
  }

  // ---- editing -----------------------------------------------------------------------------------

  private isPointMode(): boolean {
    const input = this.editing?.source === "bar" ? this.formula : this.cellEditor;
    if (!input.value.startsWith("=")) return false;
    const before = input.value.slice(0, input.selectionStart ?? input.value.length).trimEnd();
    return /[=(,+\-*/^&<>:;]$/.test(before);
  }

  private refText(a: { r: number; c: number }, b: { r: number; c: number }): string {
    const r = norm(a, b);
    return r.r0 === r.r1 && r.c0 === r.c1 ? a1(r.r0, r.c0) : `${a1(r.r0, r.c0)}:${a1(r.r1, r.c1)}`;
  }

  /** Start editing the active cell; `initial` replaces the content (typing). */
  beginEdit(initial: string | null) {
    if (!this.wb) return;
    this.editing = { ...this.active, source: "cell" };
    const text = initial ?? this.wb.input(this.sheet, this.active.r, this.active.c);
    this.cellEditor.value = text;
    this.formula.value = text;
    this.cellEditor.hidden = false;
    this.placeEditor();
    this.cellEditor.focus();
    const end = text.length;
    this.cellEditor.setSelectionRange(end, end);
    this.syncEditors("cell");
  }

  private placeEditor() {
    const e = this.editing;
    if (!e) return;
    const x = this.colX(e.c);
    const y = this.rowY(e.r);
    const w = Math.max(this.cols.sizeOf(e.c), 40);
    const ht = this.rows.sizeOf(e.r);
    Object.assign(this.cellEditor.style, { left: `${x}px`, top: `${y}px`, minWidth: `${w + 1}px`, height: `${Math.max(ht + 1, this.cellEditor.value.split("\n").length * 18)}px` });
  }

  private syncEditors(from: "cell" | "bar") {
    const src = from === "cell" ? this.cellEditor : this.formula;
    const dst = from === "cell" ? this.formula : this.cellEditor;
    dst.value = src.value;
    if (from === "cell") {
      this.cellEditor.style.width = `${Math.max(this.cols.sizeOf(this.editing?.c ?? 0), src.value.length * 8 + 12)}px`;
      this.placeEditor();
    }
    this.updateAc(src);
  }

  private updateAc(input: HTMLInputElement | HTMLTextAreaElement) {
    const tok = tokenAt(input.value, input.selectionStart ?? input.value.length);
    if (!tok || tok.word.length < 1) return this.hideAc();
    const w = tok.word.toUpperCase();
    this.acItems = this.functions.filter((f) => f.startsWith(w)).slice(0, 8);
    if (!this.acItems.length || (this.acItems.length === 1 && this.acItems[0] === w)) return this.hideAc();
    this.acIndex = 0;
    this.ac.hidden = false;
    const anchorX = input === this.cellEditor ? this.colX(this.editing?.c ?? 0) : 120;
    const anchorY = input === this.cellEditor ? this.rowY((this.editing?.r ?? 0) + 1) + 2 : 0;
    Object.assign(this.ac.style, { left: `${anchorX}px`, top: `${anchorY}px` });
    this.renderAc();
  }

  private renderAc() {
    this.ac.replaceChildren(
      ...this.acItems.map((f, i) =>
        h(
          "li",
          {
            role: "option",
            class: i === this.acIndex ? "active" : "",
            "aria-selected": String(i === this.acIndex),
            onmousedown: (e: MouseEvent) => {
              e.preventDefault();
              this.acIndex = i;
              this.acceptAc(this.editing?.source === "bar" ? this.formula : this.cellEditor);
            },
          },
          f,
        ),
      ),
    );
  }

  private hideAc() {
    this.ac.hidden = true;
    this.acItems = [];
  }

  private acceptAc(input: HTMLInputElement | HTMLTextAreaElement) {
    const name = this.acItems[this.acIndex];
    const pos = input.selectionStart ?? input.value.length;
    const tok = tokenAt(input.value, pos);
    this.hideAc();
    if (!name || !tok) return;
    input.setRangeText(`${name}(`, tok.start, pos, "end");
    this.syncEditors(input === this.cellEditor ? "cell" : "bar");
    this.hideAc();
  }

  /** F4: cycle A1 → $A$1 → A$1 → $A1 for the reference at the caret. */
  private toggleAbsolute(input: HTMLInputElement | HTMLTextAreaElement) {
    const pos = input.selectionStart ?? 0;
    const v = input.value;
    const re = /\$?[A-Z]{1,3}\$?\d+/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(v))) {
      if (pos >= m.index && pos <= m.index + m[0].length) {
        const bare = m[0].replace(/\$/g, "");
        const [, col, row] = /([A-Z]+)(\d+)/.exec(bare)!;
        const cur = m[0];
        const next = cur === bare ? `$${col}$${row}` : cur === `$${col}$${row}` ? `${col}$${row}` : cur === `${col}$${row}` ? `$${col}${row}` : bare;
        input.setRangeText(next, m.index, m.index + cur.length, "end");
        this.syncEditors(input === this.cellEditor ? "cell" : "bar");
        return;
      }
    }
  }

  /** Commit the editor and move the active cell by (dr, dc). */
  commit(dr: number, dc: number) {
    const e = this.editing;
    if (!e || !this.wb) return;
    const text = (e.source === "cell" ? this.cellEditor : this.formula).value;
    const before = this.wb.input(this.sheet, e.r, e.c);
    this.hideAc();
    if (text !== before) {
      const ch = this.applyEdits([{ sheet: this.sheet, r: e.r, c: e.c, text }]);
      if (ch?.error) {
        // keep the editor open so the formula can be fixed
        (e.source === "cell" ? this.cellEditor : this.formula).focus();
        return;
      }
    }
    this.endEdit();
    let col = e.c + dc;
    if (dc !== 0) this.tabStart ??= e.c;
    else if (dr > 0 && this.tabStart !== null) {
      col = this.tabStart;
      this.tabStart = null;
    } else this.tabStart = null;
    this.selectCell(Math.min(MAX_ROWS - 1, Math.max(0, e.r + dr)), Math.min(MAX_COLS - 1, Math.max(0, col)));
    this.focus();
  }

  cancelEdit() {
    this.endEdit();
    this.selectionChanged();
    this.focus();
  }

  private endEdit() {
    this.editing = null;
    this.cellEditor.hidden = true;
    this.hideAc();
  }

  get isEditing(): boolean {
    return this.editing !== null;
  }

  private cellsOf(r: Rect): Edit[] {
    const out: Edit[] = [];
    const used = this.info[this.sheet]?.used;
    const r1 = Math.min(r.r1, Math.max(used?.r1 ?? 0, r.r0));
    const c1 = Math.min(r.c1, Math.max(used?.c1 ?? 0, r.c0));
    for (let i = r.r0; i <= r1; i++) for (let j = r.c0; j <= c1; j++) out.push({ sheet: this.sheet, r: i, c: j, text: this.wb!.input(this.sheet, i, j) });
    return out;
  }

  private recordUndo(old: Edit[]) {
    this.undoStack.push(old);
    if (this.undoStack.length > 200) this.undoStack.shift();
    this.redoStack = [];
  }

  /** Apply edits with undo. Returns the engine's change report. */
  applyEdits(edits: Edit[]): Change | null {
    if (!this.wb || !edits.length) return null;
    const old = edits.map((e) => ({ ...e, text: this.wb!.input(e.sheet, e.r, e.c) }));
    const ch = this.wb.edit(edits);
    if (ch.error) {
      this.opts.onError?.(ch.error);
      // roll back partial batches
      if (edits.length > 1) this.wb.edit(old);
      return ch;
    }
    this.recordUndo(old);
    this.changed(ch);
    this.selectionChanged();
    return ch;
  }

  private clearSelection() {
    const edits = this.cellsOf(this.selection).filter((e) => e.text !== "").map((e) => ({ ...e, text: "" }));
    this.applyEdits(edits);
  }

  // ---- clipboard ---------------------------------------------------------------------------------

  /** Selection as tab-separated display text. */
  selectionText(): string {
    const s = this.selection;
    const used = this.info[this.sheet]?.used;
    const r1 = Math.min(s.r1, used?.r1 ?? s.r0);
    const c1 = Math.min(s.c1, used?.c1 ?? s.c0);
    const cells = new Map(this.wb!.view(this.sheet, { r0: s.r0, c0: s.c0, r1, c1 }).map((v) => [key(v.r, v.c), v.t]));
    const lines: string[] = [];
    for (let r = s.r0; r <= r1; r++) {
      const row: string[] = [];
      for (let c = s.c0; c <= c1; c++) {
        const t = cells.get(key(r, c)) ?? "";
        row.push(/[\t\n"]/.test(t) ? `"${t.replace(/"/g, '""')}"` : t);
      }
      lines.push(row.join("\t"));
    }
    return lines.join("\n");
  }

  private onCopy(e: ClipboardEvent, cut: boolean) {
    if (!this.wb || this.editing) return;
    e.preventDefault();
    e.clipboardData?.setData("text/plain", this.selectionText());
    this.clip = { sheet: this.sheet, rect: this.selection, cut };
    this.requestDraw();
  }

  private onPaste(e: ClipboardEvent) {
    if (!this.wb || this.editing) return;
    e.preventDefault();
    const text = e.clipboardData?.getData("text/plain") ?? "";
    this.pasteText(text);
  }

  /** Paste: internal copies keep formulas (shifted); external text is split as TSV. */
  pasteText(text: string) {
    if (!this.wb) return;
    const at = { r: this.selection.r0, c: this.selection.c0 };
    if (this.clip && text === this.clipText()) {
      const { sheet, rect, cut } = this.clip;
      const h = rect.r1 - rect.r0;
      const w = rect.c1 - rect.c0;
      const dst: Rect = { r0: at.r, c0: at.c, r1: at.r + h, c1: at.c + w };
      const old = this.cellsOf(dst);
      if (cut) old.push(...this.withSheet(sheet, () => this.cellsOf(rect)));
      const ch = this.wb.paste(sheet, rect, this.sheet, at.r, at.c);
      if (cut) {
        const clear: Edit[] = [];
        for (let r = rect.r0; r <= rect.r1; r++)
          for (let c = rect.c0; c <= rect.c1; c++) {
            const inside = sheet === this.sheet && r >= dst.r0 && r <= dst.r1 && c >= dst.c0 && c <= dst.c1;
            if (!inside) clear.push({ sheet, r, c, text: "" });
          }
        this.wb.edit(clear);
        this.clip = null;
      }
      this.recordUndo(old);
      this.changed(ch);
      this.anchor = at;
      this.active = at;
      this.extent = { r: dst.r1, c: dst.c1 };
      this.selectionChanged();
      return;
    }
    const rows = parseTsv(text);
    const edits: Edit[] = [];
    rows.forEach((row, i) => row.forEach((t, j) => edits.push({ sheet: this.sheet, r: at.r + i, c: at.c + j, text: t })));
    this.applyEdits(edits);
  }

  private clipText(): string {
    if (!this.clip) return "";
    const saved = { sheet: this.sheet, a: this.anchor, x: this.extent };
    this.sheet = this.clip.sheet;
    this.anchor = { r: this.clip.rect.r0, c: this.clip.rect.c0 };
    this.extent = { r: this.clip.rect.r1, c: this.clip.rect.c1 };
    const t = this.selectionText();
    this.sheet = saved.sheet;
    this.anchor = saved.a;
    this.extent = saved.x;
    return t;
  }

  private withSheet<T>(sheet: number, f: () => T): T {
    const s = this.sheet;
    this.sheet = sheet;
    try {
      return f();
    } finally {
      this.sheet = s;
    }
  }

  // ---- commands -----------------------------------------------------------------------------------

  /** Current formatting of the active cell (ribbon state). */
  state(): { style: CellStyle; numFormat: string; frozen: boolean } {
    if (!this.wb) return { style: {}, numFormat: "General", frozen: false };
    const s = this.wb.styleOf(this.sheet, this.active.r, this.active.c);
    const [fr, fc] = this.frozen;
    return { style: s.style, numFormat: s.numFormat, frozen: fr + fc > 0 };
  }

  private style(patch: Partial<Record<keyof CellStyle, unknown>>, numFormat: string | null = null) {
    if (!this.wb) return;
    const s = this.selection;
    const used = this.info[this.sheet]?.used;
    // whole rows/columns are clipped to the used range
    const rect = { ...s, r1: s.r1 >= MAX_ROWS - 1 ? Math.max(used?.r1 ?? 0, s.r0) : s.r1, c1: s.c1 >= MAX_COLS - 1 ? Math.max(used?.c1 ?? 0, s.c0) : s.c1 };
    try {
      this.wb.applyStyle(this.sheet, rect, numFormat, patch);
    } catch (err) {
      this.opts.onError?.(String(err instanceof Error ? err.message : err));
      return;
    }
    this.changed({ changed: [], evaluated: 0, circular: [], asyncRequests: 0 });
    this.selectionChanged();
  }

  execute(command: string, arg?: string): boolean {
    if (!this.wb) return false;
    const [cmd, inline] = command.split(":", 2) as [string, string | undefined];
    const value = arg ?? inline;
    const st = this.state().style;
    switch (cmd) {
      case "bold":
      case "italic":
      case "underline":
      case "strike":
      case "wrap":
        this.style({ [cmd]: !st[cmd as "bold"] });
        return true;
      case "alignLeft":
      case "alignCenter":
      case "alignRight": {
        const a = cmd.slice(5).toLowerCase();
        this.style({ hAlign: st.hAlign === a ? null : a });
        return true;
      }
      case "fill":
        this.style({ fill: value || null });
        return true;
      case "color":
        this.style({ color: value || null });
        return true;
      case "border":
        this.style({ border: st.border ? 0 : 15 });
        return true;
      case "numfmt":
        this.style({}, value ?? "General");
        return true;
      case "fontSize":
        this.style({ fontSize: value ? Number(value) : null });
        return true;
      case "freeze": {
        const [fr, fc] = this.frozen;
        if (fr + fc > 0) this.wb.setFrozen(this.sheet, 0, 0);
        else this.wb.setFrozen(this.sheet, this.active.r, this.active.c);
        this.changed({ changed: [], evaluated: 0, circular: [], asyncRequests: 0 });
        return true;
      }
      case "addSheet": {
        this.sheet = this.wb.addSheet(value || `Sheet${this.info.length + 1}`);
        this.changed({ changed: [], evaluated: 0, circular: [], asyncRequests: 0 });
        this.selectCell(0, 0);
        return true;
      }
      case "sheet":
        this.switchSheet(Number(value));
        return true;
      case "insert":
        this.beginEdit(value ?? "=");
        return true;
      case "chart":
        if (this.selectedChart !== null) this.editChart({ kind: (value as ChartKind) || "column" });
        else this.insertChart((value as ChartKind) || "column");
        return true;
      case "chartTitle":
        if (this.selectedChart !== null) this.editChart({ title: value ?? "" });
        return true;
      case "refreshPivots":
        try {
          this.changed(this.wb.refreshPivots());
        } catch (err) {
          this.opts.onError?.(String(err instanceof Error ? err.message : err));
        }
        return true;
      case "clear":
        this.clearSelection();
        return true;
      case "selectAll":
        this.anchor = { r: 0, c: 0 };
        this.extent = { r: MAX_ROWS - 1, c: MAX_COLS - 1 };
        this.selectionChanged();
        return true;
      case "fillDown":
      case "fillRight": {
        const s = this.selection;
        const down = cmd === "fillDown";
        if ((down && s.r0 === s.r1) || (!down && s.c0 === s.c1)) return true;
        const src = down ? { ...s, r1: s.r0 } : { ...s, c1: s.c0 };
        this.recordUndo(this.cellsOf(s));
        this.changed(this.wb.fill(this.sheet, src, s));
        this.selectionChanged();
        return true;
      }
      case "autoSum": {
        const { r, c } = this.active;
        let top = r - 1;
        const isNum = (rr: number) => this.wb!.view(this.sheet, { r0: rr, c0: c, r1: rr, c1: c })[0]?.k === "n";
        while (top >= 0 && isNum(top)) top--;
        const range = top + 1 <= r - 1 ? `${a1(top + 1, c)}:${a1(r - 1, c)}` : "";
        this.beginEdit(`=SUM(${range})`);
        return true;
      }
      case "copy":
        this.clip = { sheet: this.sheet, rect: this.selection, cut: false };
        return true;
      case "cut":
        this.clip = { sheet: this.sheet, rect: this.selection, cut: true };
        return true;
      case "paste":
        if (this.clip) this.pasteText(this.clipText());
        return true;
      case "undo":
      case "redo": {
        const from = cmd === "undo" ? this.undoStack : this.redoStack;
        const to = cmd === "undo" ? this.redoStack : this.undoStack;
        const batch = from.pop();
        if (!batch) return true;
        to.push(batch.map((e) => ({ ...e, text: this.wb!.input(e.sheet, e.r, e.c) })));
        this.changed(this.wb.edit(batch));
        this.selectionChanged();
        return true;
      }
    }
    return false;
  }

  // ---- charts, import ----------------------------------------------------------------------------

  private chartAt(x: number, y: number): number {
    for (let i = this.charts.length - 1; i >= 0; i--) {
      const a = this.charts[i]!.chart.anchor;
      if (x >= this.colX(a.c0) && x <= this.colX(a.c1 + 1) && y >= this.rowY(a.r0) && y <= this.rowY(a.r1 + 1)) return i;
    }
    return -1;
  }

  /** Change the selected chart (file charts become editor-owned). */
  editChart(patch: Partial<ChartDef>) {
    if (!this.wb || this.selectedChart === null) return;
    const cur = this.wb.charts(this.sheet)[this.selectedChart]?.chart;
    if (!cur) return;
    this.wb.updateChart(this.sheet, this.selectedChart, { ...cur, ...patch, fromFile: false });
    this.changed({ changed: [], evaluated: 0, circular: [], asyncRequests: 0 });
  }

  /** Add a classic PivotTable; returns an error message or null. */
  addPivot(sheet: number, def: PivotDef): string | null {
    if (!this.wb) return "no workbook";
    try {
      this.changed(this.wb.addPivot(sheet, def));
      return null;
    } catch (err) {
      return String(err instanceof Error ? err.message : err);
    }
  }

  /** Insert a chart of the selected data next to it. */
  insertChart(kind: ChartKind, title = ""): number | null {
    if (!this.wb) return null;
    const s = this.selection;
    const used = this.info[this.sheet]?.used;
    let data = { ...s, r1: Math.min(s.r1, used?.r1 ?? s.r1), c1: Math.min(s.c1, used?.c1 ?? s.c1) };
    if (data.r0 === data.r1 && data.c0 === data.c1 && used) data = used; // single cell: whole table
    const anchor = { r0: data.r0, c0: data.c1 + 2, r1: data.r0 + 15, c1: data.c1 + 8 };
    const i = this.wb.addChart(this.sheet, { kind, title, data, anchor });
    this.selectedChart = i;
    this.changed({ changed: [], evaluated: 0, circular: [], asyncRequests: 0 });
    return i;
  }

  /** Data connections (saved with the workbook; re-run by Refresh All). */
  get connections(): DataConnection[] {
    return this.wb?.connections<DataConnection>() ?? [];
  }

  /** Put imported rows on a new sheet. Text that looks like a formula stays text. */
  importRows(name: string, rows: unknown[][], connection?: Omit<DataConnection, "sheet">): number {
    if (!this.wb) return -1;
    const sheet = this.wb.addSheet(name.slice(0, 31).replace(/[\\/?*[\]:]/g, "_") || "Data");
    if (connection) this.wb.setConnections([...this.connections, { ...connection, sheet: this.wb.sheets()[sheet]!.name }]);
    this.writeRows(sheet, rows);
    return sheet;
  }

  /** Replace a sheet's contents with rows (Refresh). */
  replaceRows(sheetName: string, rows: unknown[][]): boolean {
    if (!this.wb) return false;
    const sheet = this.wb.sheets().findIndex((s) => s.name === sheetName);
    if (sheet < 0) return false;
    const used = this.wb.sheets()[sheet]!.used;
    if (used) {
      const clear: Edit[] = [];
      for (let r = used.r0; r <= used.r1; r++) for (let c = used.c0; c <= used.c1; c++) clear.push({ sheet, r, c, text: "" });
      this.wb.edit(clear);
    }
    this.writeRows(sheet, rows);
    return true;
  }

  private writeRows(sheet: number, rows: unknown[][]) {
    if (!this.wb) return;
    const edits: Edit[] = [];
    rows.forEach((row, r) =>
      row.forEach((v, c) => {
        if (v === null || v === undefined || v === "") return;
        let text = typeof v === "string" ? v : typeof v === "object" ? JSON.stringify(v) : String(v);
        if (/^[=+\-@]/.test(text) && typeof v === "string" && Number.isNaN(Number(text))) text = `'${text}`;
        edits.push({ sheet, r, c, text });
      }),
    );
    this.sheet = sheet;
    this.undoStack = [];
    this.redoStack = [];
    if (edits.length) this.changed(this.wb.edit(edits));
    else this.refreshInfo();
    this.selectCell(0, 0);
  }

  // ---- sheet tabs ----------------------------------------------------------------------------------

  switchSheet(i: number) {
    if (!this.wb || i < 0 || i >= this.info.length || i === this.sheet) return;
    if (this.editing) this.commit(0, 0);
    this.sheet = i;
    this.scroll.scrollTop = 0;
    this.scroll.scrollLeft = 0;
    this.refreshInfo();
    this.selectCell(0, 0);
  }

  private renderTabs() {
    const tabs = this.info.map((s, i) =>
      s.hidden
        ? null
        : h(
            "button",
            {
              class: `sheet-tab${i === this.sheet ? " active" : ""}`,
              role: "tab",
              "aria-selected": String(i === this.sheet),
              onclick: () => this.switchSheet(i),
              ondblclick: () => {
                const name = window.prompt("Rename sheet", s.name);
                if (!name || !this.wb) return;
                try {
                  this.changed(this.wb.renameSheet(i, name));
                } catch (err) {
                  this.opts.onError?.(String(err instanceof Error ? err.message : err));
                }
              },
              oncontextmenu: (e: MouseEvent) => {
                e.preventDefault();
                if (!this.wb || this.info.length < 2 || !window.confirm(`Delete sheet "${s.name}"?`)) return;
                try {
                  this.changed(this.wb.deleteSheet(i));
                  this.undoStack = [];
                  this.redoStack = [];
                } catch (err) {
                  this.opts.onError?.(String(err instanceof Error ? err.message : err));
                }
              },
            },
            s.name,
          ),
    );
    this.tabsBar.replaceChildren(...tabs.filter((t): t is HTMLButtonElement => !!t), h("button", { class: "sheet-tab add", title: "New sheet", "aria-label": "New sheet", onclick: () => this.execute("addSheet") }, "+"));
  }

  // ---- drawing -------------------------------------------------------------------------------------

  requestDraw() {
    if (this.drawQueued || this.disposed) return;
    this.drawQueued = true;
    const raf = typeof requestAnimationFrame === "function" ? requestAnimationFrame : (f: () => void) => setTimeout(f, 16);
    raf(() => {
      this.drawQueued = false;
      this.draw();
    });
  }

  /** Visible index range on an axis for the scrolled region. */
  private visible(axis: Axis, frozen: number, scrollPos: number, extent: number): [number, number] {
    const start = axis.at(axis.offset(frozen) + scrollPos);
    const end = axis.at(axis.offset(frozen) + scrollPos + extent);
    return [Math.max(frozen, start), Math.min(axis.count - 1, end + 1)];
  }

  draw() {
    if (!this.wb || this.disposed) return;
    const W = this.main.clientWidth;
    const H = this.main.clientHeight;
    if (!W || !H) return;
    const dpr = window.devicePixelRatio || 1;
    if (this.canvas.width !== Math.round(W * dpr) || this.canvas.height !== Math.round(H * dpr)) {
      this.canvas.width = Math.round(W * dpr);
      this.canvas.height = Math.round(H * dpr);
      this.canvas.style.width = `${W}px`;
      this.canvas.style.height = `${H}px`;
    }
    const ctx = this.canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, W, H);
    const [fr, fc] = this.frozen;
    const fw = this.cols.offset(fc);
    const fh = this.rows.offset(fr);
    const [vr0, vr1] = this.visible(this.rows, fr, this.scrollY, H - HDR_H - fh);
    const [vc0, vc1] = this.visible(this.cols, fc, this.scroll.scrollLeft, W - this.hdrW - fw);
    const rowSets: [number, number][] = fr > 0 ? [[0, fr - 1], [vr0, vr1]] : [[vr0, vr1]];
    const colSets: [number, number][] = fc > 0 ? [[0, fc - 1], [vc0, vc1]] : [[vc0, vc1]];
    for (const [r0, r1] of rowSets) {
      for (const [c0, c1] of colSets) {
        const clipX = c0 < fc ? this.hdrW : this.hdrW + fw;
        const clipY = r0 < fr ? HDR_H : HDR_H + fh;
        const clipW = c0 < fc ? fw : W - clipX;
        const clipH = r0 < fr ? fh : H - clipY;
        ctx.save();
        ctx.beginPath();
        ctx.rect(clipX, clipY, clipW, clipH);
        ctx.clip();
        this.drawCells(ctx, r0, r1, c0, c1);
        this.drawSelection(ctx);
        ctx.restore();
      }
    }
    this.charts = this.wb.charts(this.sheet);
    if (this.chartDrag && this.charts[this.chartDrag.index]) this.charts[this.chartDrag.index]!.chart.anchor = this.chartDrag.anchor;
    if (this.charts.length) {
      ctx.save();
      ctx.beginPath();
      ctx.rect(this.hdrW, HDR_H, W - this.hdrW, H - HDR_H);
      ctx.clip();
      this.charts.forEach(({ chart, data }, i) => {
        const x0 = this.colX(chart.anchor.c0);
        const y0 = this.rowY(chart.anchor.r0);
        const x1 = this.colX(chart.anchor.c1 + 1);
        const y1 = this.rowY(chart.anchor.r1 + 1);
        if (x1 < this.hdrW || y1 < HDR_H || x0 > W || y0 > H) return;
        drawChart(ctx, x0, y0, x1 - x0, y1 - y0, chart.kind, chart.title, data, this.selectedChart === i);
        if (this.selectedChart === i) {
          ctx.fillStyle = ACCENT.base;
          ctx.fillRect(x1 - 6, y1 - 6, 8, 8);
        }
      });
      ctx.restore();
    }
    this.drawHeaders(ctx, W, H, rowSets, colSets);
    if (fr > 0 || fc > 0) {
      ctx.strokeStyle = "#9ca3af";
      ctx.lineWidth = 1;
      ctx.beginPath();
      if (fr > 0) {
        ctx.moveTo(0, HDR_H + fh + 0.5);
        ctx.lineTo(W, HDR_H + fh + 0.5);
      }
      if (fc > 0) {
        ctx.moveTo(this.hdrW + fw + 0.5, 0);
        ctx.lineTo(this.hdrW + fw + 0.5, H);
      }
      ctx.stroke();
    }
  }

  private drawCells(ctx: CanvasRenderingContext2D, r0: number, r1: number, c0: number, c1: number) {
    const cells = this.wb!.view(this.sheet, { r0, c0, r1, c1 });
    const map = new Map<number, ViewCell>();
    for (const v of cells) map.set(key(v.r, v.c), v);
    const merges = (this.info[this.sheet]?.merges ?? []).filter((m) => m.r1 >= r0 && m.r0 <= r1 && m.c1 >= c0 && m.c0 <= c1);
    const inMerge = (r: number, c: number) => merges.find((m) => r >= m.r0 && r <= m.r1 && c >= m.c0 && c <= m.c1);
    // fills
    for (const v of cells) {
      const st = this.styles[v.s ?? 0];
      if (!st?.fill) continue;
      ctx.fillStyle = st.fill;
      ctx.fillRect(this.colX(v.c), this.rowY(v.r), this.cols.sizeOf(v.c), this.rows.sizeOf(v.r));
    }
    // gridlines
    ctx.strokeStyle = "#e5e7eb";
    ctx.lineWidth = 1;
    ctx.beginPath();
    const yTop = this.rowY(r0);
    const yBot = this.rowY(r1 + 1);
    const xL = this.colX(c0);
    const xR = this.colX(c1 + 1);
    for (let c = c0; c <= c1 + 1; c++) {
      const x = Math.round(this.colX(c)) - 0.5;
      ctx.moveTo(x, yTop);
      ctx.lineTo(x, yBot);
    }
    for (let r = r0; r <= r1 + 1; r++) {
      const y = Math.round(this.rowY(r)) - 0.5;
      ctx.moveTo(xL, y);
      ctx.lineTo(xR, y);
    }
    ctx.stroke();
    // merged areas cover gridlines
    for (const m of merges) {
      const st = this.styles[map.get(key(m.r0, m.c0))?.s ?? 0];
      ctx.fillStyle = st?.fill ?? "#ffffff";
      ctx.fillRect(this.colX(m.c0), this.rowY(m.r0), this.colX(m.c1 + 1) - this.colX(m.c0) - 1, this.rowY(m.r1 + 1) - this.rowY(m.r0) - 1);
    }
    // text
    ctx.textBaseline = "middle";
    for (const v of cells) {
      if (!v.t) continue;
      const m = inMerge(v.r, v.c);
      if (m && (m.r0 !== v.r || m.c0 !== v.c)) continue;
      const st = this.styles[v.s ?? 0] ?? {};
      const x0 = this.colX(v.c);
      const y0 = this.rowY(v.r);
      let w = m ? this.colX(m.c1 + 1) - x0 : this.cols.sizeOf(v.c);
      const ht = m ? this.rowY(m.r1 + 1) - y0 : this.rows.sizeOf(v.r);
      const align = st.hAlign ?? (v.k === "n" ? "right" : v.k === "b" || v.k === "e" ? "center" : "left");
      // text overflows into empty neighbours (Excel), numbers never do
      if (!m && v.k === "s" && align === "left" && !st.wrap) {
        let c = v.c + 1;
        while (c <= c1 && !map.has(key(v.r, c)) && w < 2000) w += this.cols.sizeOf(c++);
      }
      const sizePx = ((st.fontSize ?? 11) * 96) / 72;
      ctx.font = `${st.italic ? "italic " : ""}${st.bold ? "bold " : ""}${sizePx.toFixed(1)}px ${st.fontName ? `"${st.fontName}", ` : ""}Calibri, Carlito, "Segoe UI", system-ui, sans-serif`;
      ctx.fillStyle = v.color ? cssColor(v.color) : st.color ?? (v.k === "e" ? "#b91c1c" : "#111827");
      ctx.save();
      ctx.beginPath();
      ctx.rect(x0 + 1, y0, w - 2, ht);
      ctx.clip();
      let text = v.t;
      if (v.k === "n" && ctx.measureText(text).width > w - 6 && !m) text = "#".repeat(Math.max(1, Math.floor((w - 6) / Math.max(1, ctx.measureText("#").width))));
      const lines = st.wrap ? wrapText(ctx, text, w - 6) : [text];
      const lineH = sizePx * 1.2;
      const blockH = lines.length * lineH;
      const vAlign = st.vAlign ?? "bottom";
      let ty = vAlign === "top" ? y0 + lineH / 2 + 1 : vAlign === "center" ? y0 + (ht - blockH) / 2 + lineH / 2 : y0 + ht - blockH + lineH / 2 - 2;
      for (const line of lines) {
        ctx.textAlign = align === "right" ? "right" : align === "center" ? "center" : "left";
        const tx = align === "right" ? x0 + w - 3 : align === "center" ? x0 + w / 2 : x0 + 3;
        ctx.fillText(line, tx, ty);
        if (st.underline || st.strike) {
          const tw = ctx.measureText(line).width;
          const lx = align === "right" ? tx - tw : align === "center" ? tx - tw / 2 : tx;
          const ly = st.underline ? ty + sizePx * 0.45 : ty;
          ctx.fillRect(lx, ly, tw, 1);
        }
        ty += lineH;
      }
      ctx.restore();
    }
    // pictures from =PY cells, fitted inside the cell
    for (const [k, pic] of this.images) {
      const [s, rr, cc] = k.split(":").map(Number) as [number, number, number];
      if (s !== this.sheet || rr < r0 || rr > r1 || cc < c0 || cc > c1 || !pic.img.complete) continue;
      const x0 = this.colX(cc);
      const y0 = this.rowY(rr);
      const cw = this.cols.sizeOf(cc) - 2;
      const ch = this.rows.sizeOf(rr) - 2;
      const scale = Math.min(cw / pic.w, ch / pic.h);
      const dw = pic.w * scale;
      const dh = pic.h * scale;
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(x0 + 1, y0 + 1, cw, ch);
      try {
        ctx.drawImage(pic.img, x0 + 1 + (cw - dw) / 2, y0 + 1 + (ch - dh) / 2, dw, dh);
      } catch {
        // image not decodable yet
      }
    }
    // borders
    ctx.strokeStyle = "#111827";
    ctx.beginPath();
    for (const v of cells) {
      const b = this.styles[v.s ?? 0]?.border ?? 0;
      if (!b) continue;
      const x0 = Math.round(this.colX(v.c)) - 0.5;
      const y0 = Math.round(this.rowY(v.r)) - 0.5;
      const x1 = Math.round(this.colX(v.c + 1)) - 0.5;
      const y1 = Math.round(this.rowY(v.r + 1)) - 0.5;
      if (b & 1) (ctx.moveTo(x0, y0), ctx.lineTo(x0, y1));
      if (b & 2) (ctx.moveTo(x1, y0), ctx.lineTo(x1, y1));
      if (b & 4) (ctx.moveTo(x0, y0), ctx.lineTo(x1, y0));
      if (b & 8) (ctx.moveTo(x0, y1), ctx.lineTo(x1, y1));
    }
    ctx.stroke();
    // spill outlines (dashed blue like Excel)
    for (const sp of this.info[this.sheet]?.spills ?? []) {
      if (sp.r1 < r0 || sp.r0 > r1 || sp.c1 < c0 || sp.c0 > c1) continue;
      const a = this.active;
      if (a.r < sp.r0 || a.r > sp.r1 || a.c < sp.c0 || a.c > sp.c1) continue;
      ctx.save();
      ctx.setLineDash([3, 2]);
      ctx.strokeStyle = "#3b82f6";
      ctx.strokeRect(this.colX(sp.c0) + 0.5, this.rowY(sp.r0) + 0.5, this.colX(sp.c1 + 1) - this.colX(sp.c0) - 1, this.rowY(sp.r1 + 1) - this.rowY(sp.r0) - 1);
      ctx.restore();
    }
  }

  private drawSelection(ctx: CanvasRenderingContext2D) {
    const s = this.selection;
    const x0 = this.colX(s.c0);
    const y0 = this.rowY(s.r0);
    const x1 = this.colX(Math.min(s.c1 + 1, MAX_COLS));
    const y1 = this.rowY(Math.min(s.r1 + 1, MAX_ROWS));
    const multi = s.r0 !== s.r1 || s.c0 !== s.c1;
    if (multi) {
      ctx.fillStyle = "rgba(5,150,105,0.10)";
      ctx.fillRect(x0, y0, x1 - x0, y1 - y0);
    }
    ctx.strokeStyle = ACCENT.base;
    ctx.lineWidth = 2;
    ctx.strokeRect(x0, y0, x1 - x0 - 1, y1 - y0 - 1);
    if (this.fillTarget) {
      const f = this.fillTarget;
      ctx.save();
      ctx.setLineDash([4, 3]);
      ctx.lineWidth = 1;
      ctx.strokeRect(this.colX(f.c0), this.rowY(f.r0), this.colX(f.c1 + 1) - this.colX(f.c0) - 1, this.rowY(f.r1 + 1) - this.rowY(f.r0) - 1);
      ctx.restore();
    }
    if (this.clip && this.clip.sheet === this.sheet) {
      const c = this.clip.rect;
      ctx.save();
      ctx.setLineDash([5, 3]);
      ctx.lineWidth = 1.5;
      ctx.strokeRect(this.colX(c.c0) + 1, this.rowY(c.r0) + 1, this.colX(c.c1 + 1) - this.colX(c.c0) - 3, this.rowY(c.r1 + 1) - this.rowY(c.r0) - 3);
      ctx.restore();
    }
    // fill handle
    ctx.fillStyle = ACCENT.base;
    ctx.fillRect(x1 - 4, y1 - 4, 6, 6);
    ctx.lineWidth = 1;
  }

  private drawHeaders(ctx: CanvasRenderingContext2D, W: number, H: number, rowSets: [number, number][], colSets: [number, number][]) {
    const s = this.selection;
    ctx.fillStyle = "#f3f4f6";
    ctx.fillRect(0, 0, W, HDR_H);
    ctx.fillRect(0, 0, this.hdrW, H);
    ctx.font = "12px system-ui, sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.strokeStyle = "#d1d5db";
    ctx.lineWidth = 1;
    const [fr, fc] = this.frozen;
    for (const [c0, c1] of colSets) {
      ctx.save();
      ctx.beginPath();
      ctx.rect(c0 < fc ? this.hdrW : this.hdrW + this.cols.offset(fc), 0, W, HDR_H);
      ctx.clip();
      for (let c = c0; c <= c1; c++) {
        const x = this.colX(c);
        const w = this.cols.sizeOf(c);
        const on = c >= s.c0 && c <= s.c1;
        if (on) {
          ctx.fillStyle = ACCENT.soft;
          ctx.fillRect(x, 0, w, HDR_H);
        }
        ctx.fillStyle = on ? ACCENT.strong : "#4b5563";
        ctx.fillText(colName(c), x + w / 2, HDR_H / 2);
        ctx.beginPath();
        ctx.moveTo(Math.round(x + w) - 0.5, 0);
        ctx.lineTo(Math.round(x + w) - 0.5, HDR_H);
        ctx.stroke();
      }
      ctx.restore();
    }
    for (const [r0, r1] of rowSets) {
      ctx.save();
      ctx.beginPath();
      ctx.rect(0, r0 < fr ? HDR_H : HDR_H + this.rows.offset(fr), this.hdrW, H);
      ctx.clip();
      for (let r = r0; r <= r1; r++) {
        const y = this.rowY(r);
        const ht = this.rows.sizeOf(r);
        const on = r >= s.r0 && r <= s.r1;
        if (on) {
          ctx.fillStyle = ACCENT.soft;
          ctx.fillRect(0, y, this.hdrW, ht);
        }
        ctx.fillStyle = on ? ACCENT.strong : "#4b5563";
        ctx.fillText(String(r + 1), this.hdrW / 2, y + ht / 2);
        ctx.beginPath();
        ctx.moveTo(0, Math.round(y + ht) - 0.5);
        ctx.lineTo(this.hdrW, Math.round(y + ht) - 0.5);
        ctx.stroke();
      }
      ctx.restore();
    }
    ctx.beginPath();
    ctx.moveTo(0, HDR_H - 0.5);
    ctx.lineTo(W, HDR_H - 0.5);
    ctx.moveTo(this.hdrW - 0.5, 0);
    ctx.lineTo(this.hdrW - 0.5, H);
    ctx.stroke();
  }
}

function fmt(n: number): string {
  return Number.isInteger(n) ? n.toLocaleString("en-US") : n.toLocaleString("en-US", { maximumFractionDigits: 6 });
}

function cssColor(c: string): string {
  const named: Record<string, string> = { red: "#dc2626", blue: "#2563eb", green: "#16a34a", magenta: "#c026d3", cyan: "#0891b2", yellow: "#ca8a04", white: "#ffffff", black: "#111827" };
  return named[c] ?? c;
}

function wrapText(ctx: CanvasRenderingContext2D, text: string, width: number): string[] {
  const out: string[] = [];
  for (const para of text.split("\n")) {
    let line = "";
    for (const word of para.split(/(\s+)/)) {
      const next = line + word;
      if (line && ctx.measureText(next).width > width) {
        out.push(line.trimEnd());
        line = word.trimStart();
      } else line = next;
    }
    out.push(line);
  }
  return out;
}

/** Parse tab-separated text (Excel clipboard format, quoted fields). */
export function parseTsv(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cur = "";
  let q = false;
  const t = text.replace(/\r\n?/g, "\n").replace(/\n$/, "");
  for (let i = 0; i < t.length; i++) {
    const ch = t[i]!;
    if (q) {
      if (ch === '"' && t[i + 1] === '"') {
        cur += '"';
        i++;
      } else if (ch === '"') q = false;
      else cur += ch;
    } else if (ch === '"' && cur === "") q = true;
    else if (ch === "\t") {
      row.push(cur);
      cur = "";
    } else if (ch === "\n") {
      row.push(cur);
      rows.push(row);
      row = [];
      cur = "";
    } else cur += ch;
  }
  row.push(cur);
  rows.push(row);
  return rows;
}
