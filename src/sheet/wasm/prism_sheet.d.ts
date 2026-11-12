/* tslint:disable */
/* eslint-disable */

export class SheetEngine {
    free(): void;
    [Symbol.dispose](): void;
    addChart(sheet: number, def_json: string): number;
    addSheet(name: string): number;
    applyStyle(sheet: number, r0: number, c0: number, r1: number, c1: number, num_format: string | null | undefined, patch_json: string): void;
    /**
     * Pending `=PY` requests.
     */
    asyncRequests(): string;
    /**
     * `[{chart, data}]` for a sheet.
     */
    charts(sheet: number): string;
    deleteSheet(sheet: number, now: number): string;
    /**
     * `[{sheet,r,c,text}]` → change report.
     */
    edit(edits_json: string, now: number): string;
    fill(sheet: number, sr0: number, sc0: number, sr1: number, sc1: number, dr0: number, dc0: number, dr1: number, dc1: number, now: number): string;
    static functionNames(): string;
    input(sheet: number, r: number, c: number): string;
    model(): string;
    /**
     * Load a `WorkbookModel` (JSON) and calculate it. `now` is an Excel serial.
     */
    constructor(model_json: string, now: number);
    paste(src_sheet: number, sr0: number, sc0: number, sr1: number, sc1: number, dst_sheet: number, dr: number, dc: number, now: number): string;
    recalc(now: number): string;
    removeChart(sheet: number, index: number): void;
    renameSheet(sheet: number, name: string, now: number): string;
    /**
     * Negative width resets to the default.
     */
    setColWidth(sheet: number, col: number, width: number): void;
    setFrozen(sheet: number, rows: number, cols: number): void;
    setRowHeight(sheet: number, row: number, height: number): void;
    sheets(): string;
    styleOf(sheet: number, r: number, c: number): string;
    /**
     * Visual styles by style index.
     */
    styles(): string;
    /**
     * Answer a `=PY` request with a value (`1`, `"x"`, `true`, `{"e":"#VALUE!"}`) or a 2-D array.
     */
    supplyAsync(key: string, sheet: number, r: number, c: number, value_json: string, now: number): string;
    updateChart(sheet: number, index: number, def_json: string): void;
    view(sheet: number, r0: number, c0: number, r1: number, c1: number): string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_sheetengine_free: (a: number, b: number) => void;
    readonly sheetengine_addChart: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly sheetengine_addSheet: (a: number, b: number, c: number) => number;
    readonly sheetengine_applyStyle: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => [number, number];
    readonly sheetengine_asyncRequests: (a: number) => [number, number];
    readonly sheetengine_charts: (a: number, b: number) => [number, number];
    readonly sheetengine_deleteSheet: (a: number, b: number, c: number) => [number, number, number, number];
    readonly sheetengine_edit: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly sheetengine_fill: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => [number, number];
    readonly sheetengine_functionNames: () => [number, number];
    readonly sheetengine_input: (a: number, b: number, c: number, d: number) => [number, number];
    readonly sheetengine_model: (a: number) => [number, number];
    readonly sheetengine_new: (a: number, b: number, c: number) => [number, number, number];
    readonly sheetengine_paste: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => [number, number];
    readonly sheetengine_recalc: (a: number, b: number) => [number, number];
    readonly sheetengine_removeChart: (a: number, b: number, c: number) => void;
    readonly sheetengine_renameSheet: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly sheetengine_setColWidth: (a: number, b: number, c: number, d: number) => void;
    readonly sheetengine_setFrozen: (a: number, b: number, c: number, d: number) => void;
    readonly sheetengine_setRowHeight: (a: number, b: number, c: number, d: number) => void;
    readonly sheetengine_sheets: (a: number) => [number, number];
    readonly sheetengine_styleOf: (a: number, b: number, c: number, d: number) => [number, number];
    readonly sheetengine_styles: (a: number) => [number, number];
    readonly sheetengine_supplyAsync: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number, number, number];
    readonly sheetengine_updateChart: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly sheetengine_view: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
