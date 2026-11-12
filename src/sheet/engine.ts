/**
 * Typed wrapper over the WASM spreadsheet engine (crates/prism-sheet,
 * built by scripts/build-sheet-wasm.sh). All calculation happens in Rust;
 * this file only converts JSON.
 */
import init, { initSync, SheetEngine } from "./wasm/prism_sheet.js";
import type { CellStyle, ChartDef, PivotDef, Rect, WorkbookModel } from "./model";

export interface ViewCell {
  r: number;
  c: number;
  /** Formatted text. */
  t: string;
  /** n number, s text, b bool, e error, f empty formula result, - styled blank. */
  k: "n" | "s" | "b" | "e" | "f" | "-";
  s?: number;
  color?: string;
  /** Raw number of numeric cells. */
  n?: number;
  spill?: boolean;
}

export interface Change {
  changed: [number, Rect][];
  evaluated: number;
  error?: string;
  circular: [number, { row: number; col: number }][];
  asyncRequests: number;
}

export interface SheetInfo {
  name: string;
  hidden: boolean;
  used: Rect | null;
  frozen: [number, number];
  colWidths: [number, number][];
  rowHeights: [number, number][];
  merges: Rect[];
  spills: Rect[];
}

export interface ChartData {
  categories: string[];
  series: { name: string; values: (number | null)[] }[];
}

export interface AsyncRequest {
  key: string;
  sheet: number;
  row: number;
  col: number;
  function: string;
  code: string;
  returnType: number;
  inputs: [string, unknown[][]][];
}

let ready: Promise<void> | null = null;
let loaded = false;

/** Load the WASM module once (browser: fetched by URL). */
export function loadEngine(): Promise<void> {
  if (loaded) return Promise.resolve();
  ready ??= init().then(() => {
    loaded = true;
  });
  return ready;
}

/** Synchronous init from bytes (tests / workers). */
export function loadEngineSync(bytes: BufferSource): void {
  if (loaded) return;
  initSync({ module: bytes });
  loaded = true;
}

/** Excel serial date-time for "now" in local time. */
export function nowSerial(date = new Date()): number {
  return (date.getTime() - date.getTimezoneOffset() * 60_000) / 86_400_000 + 25569;
}

export class Workbook {
  private e: SheetEngine;

  constructor(model?: WorkbookModel) {
    this.e = new SheetEngine(model ? JSON.stringify(model) : "", nowSerial());
  }

  model(): WorkbookModel {
    return JSON.parse(this.e.model()) as WorkbookModel;
  }

  edit(edits: { sheet: number; r: number; c: number; text: string }[]): Change {
    return JSON.parse(this.e.edit(JSON.stringify(edits), nowSerial())) as Change;
  }

  set(sheet: number, r: number, c: number, text: string): Change {
    return this.edit([{ sheet, r, c, text }]);
  }

  recalc(): Change {
    return JSON.parse(this.e.recalc(nowSerial())) as Change;
  }

  view(sheet: number, rect: Rect): ViewCell[] {
    return JSON.parse(this.e.view(sheet, rect.r0, rect.c0, rect.r1, rect.c1)) as ViewCell[];
  }

  input(sheet: number, r: number, c: number): string {
    return this.e.input(sheet, r, c);
  }

  sheets(): SheetInfo[] {
    return JSON.parse(this.e.sheets()) as SheetInfo[];
  }

  styles(): CellStyle[] {
    return JSON.parse(this.e.styles()) as CellStyle[];
  }

  styleOf(sheet: number, r: number, c: number): { numFormat: string; style: CellStyle } {
    return JSON.parse(this.e.styleOf(sheet, r, c)) as { numFormat: string; style: CellStyle };
  }

  /** Patch keys set to null/false are removed. */
  applyStyle(sheet: number, rect: Rect, numFormat: string | null, patch: Partial<Record<keyof CellStyle, unknown>> = {}): void {
    this.e.applyStyle(sheet, rect.r0, rect.c0, rect.r1, rect.c1, numFormat ?? undefined, JSON.stringify(patch));
  }

  addSheet(name: string): number {
    return this.e.addSheet(name);
  }

  renameSheet(sheet: number, name: string): Change {
    return JSON.parse(this.e.renameSheet(sheet, name, nowSerial())) as Change;
  }

  deleteSheet(sheet: number): Change {
    return JSON.parse(this.e.deleteSheet(sheet, nowSerial())) as Change;
  }

  setColWidth(sheet: number, col: number, width: number | null): void {
    this.e.setColWidth(sheet, col, width ?? -1);
  }

  setRowHeight(sheet: number, row: number, height: number | null): void {
    this.e.setRowHeight(sheet, row, height ?? -1);
  }

  setFrozen(sheet: number, rows: number, cols: number): void {
    this.e.setFrozen(sheet, rows, cols);
  }

  fill(sheet: number, src: Rect, dst: Rect): Change {
    return JSON.parse(this.e.fill(sheet, src.r0, src.c0, src.r1, src.c1, dst.r0, dst.c0, dst.r1, dst.c1, nowSerial())) as Change;
  }

  paste(srcSheet: number, src: Rect, dstSheet: number, r: number, c: number): Change {
    return JSON.parse(this.e.paste(srcSheet, src.r0, src.c0, src.r1, src.c1, dstSheet, r, c, nowSerial())) as Change;
  }

  charts(sheet: number): { chart: ChartDef; data: ChartData }[] {
    return JSON.parse(this.e.charts(sheet)) as { chart: ChartDef; data: ChartData }[];
  }

  addChart(sheet: number, def: ChartDef): number {
    return this.e.addChart(sheet, JSON.stringify(def));
  }

  updateChart(sheet: number, index: number, def: ChartDef): void {
    this.e.updateChart(sheet, index, JSON.stringify(def));
  }

  removeChart(sheet: number, index: number): void {
    this.e.removeChart(sheet, index);
  }

  addPivot(sheet: number, def: PivotDef): Change {
    return JSON.parse(this.e.addPivot(sheet, JSON.stringify(def), nowSerial())) as Change;
  }

  refreshPivots(): Change {
    return JSON.parse(this.e.refreshPivots(nowSerial())) as Change;
  }

  removePivot(sheet: number, index: number): Change {
    return JSON.parse(this.e.removePivot(sheet, index, nowSerial())) as Change;
  }

  connections<T>(): T[] {
    return JSON.parse(this.e.connections()) as T[];
  }

  setConnections(list: unknown[]): void {
    this.e.setConnections(JSON.stringify(list));
  }

  pivots(sheet: number): PivotDef[] {
    return JSON.parse(this.e.pivots(sheet)) as PivotDef[];
  }

  asyncRequests(): AsyncRequest[] {
    return JSON.parse(this.e.asyncRequests()) as AsyncRequest[];
  }

  supplyAsync(req: AsyncRequest, value: unknown): Change {
    return JSON.parse(this.e.supplyAsync(req.key, req.sheet, req.row, req.col, JSON.stringify(value), nowSerial())) as Change;
  }

  static functionNames(): string[] {
    return JSON.parse(SheetEngine.functionNames()) as string[];
  }

  dispose(): void {
    this.e.free();
  }
}
