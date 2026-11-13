/* tslint:disable */
/* eslint-disable */

/**
 * Colour resolution against one theme + colour map (parsed once).
 */
export class ColorContext {
    free(): void;
    [Symbol.dispose](): void;
    constructor(theme_json: string, clr_map_json: string);
    /**
     * `color_json` → CSS `rgba(...)`; `ph` is the placeholder colour (`#RRGGBB`) for `phClr`.
     */
    resolve(color_json: string, ph?: string | null): string;
}

/**
 * Evaluate a `Geometry` (JSON) at a size → `{paths, textRect}` JSON.
 */
export function evaluateGeometry(geom_json: string, w: number, h: number): string;

/**
 * Theme style-matrix entries (`fillStyles` / `bgFillStyles` XML) → `Fill` JSON.
 */
export function parseFill(xml: string): string;

/**
 * Theme `lineStyles` XML (`a:ln`) → `Line` JSON.
 */
export function parseLine(xml: string): string;

/**
 * A preset's definition (adjust handles and defaults) as JSON.
 */
export function presetDefinition(name: string): string;

export function presetNames(): string;

/**
 * Resolve a colour (JSON) to CSS against a theme and colour map (JSON).
 */
export function resolveColor(color_json: string, theme_json: string, clr_map_json: string, ph_css?: string | null): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_colorcontext_free: (a: number, b: number) => void;
    readonly colorcontext_new: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly colorcontext_resolve: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly evaluateGeometry: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly parseFill: (a: number, b: number) => [number, number];
    readonly parseLine: (a: number, b: number) => [number, number];
    readonly presetDefinition: (a: number, b: number) => [number, number];
    readonly presetNames: () => [number, number];
    readonly resolveColor: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number, number];
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
