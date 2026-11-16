/* tslint:disable */
/* eslint-disable */

export class IoDocument {
    free(): void;
    [Symbol.dispose](): void;
    addImage(bytes: Uint8Array | number[]): string;
    /**
     * A new blank document ("document" | "workbook" | "presentation").
     */
    static blank(kind: string): IoDocument;
    /**
     * `[mime, bytes]` of a picture, or undefined.
     */
    image(key: string): Pair | undefined;
    /**
     * "document" | "workbook" | "presentation"
     */
    kind(): string;
    /**
     * Editable model (JSON).
     */
    model(): string;
    /**
     * Open DOCX / XLSX / PPTX bytes.
     */
    constructor(bytes: Uint8Array | number[]);
    /**
     * File bytes with the current model (`now` = Excel serial for TODAY/NOW).
     */
    save(now: number): Uint8Array;
    setModel(json: string): void;
}

export class Pair {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    get bytes(): Uint8Array;
    set bytes(value: Uint8Array | number[]);
    mime: string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_get_pair_bytes: (a: number) => [number, number];
    readonly __wbg_get_pair_mime: (a: number) => [number, number];
    readonly __wbg_iodocument_free: (a: number, b: number) => void;
    readonly __wbg_pair_free: (a: number, b: number) => void;
    readonly __wbg_set_pair_bytes: (a: number, b: number, c: number) => void;
    readonly __wbg_set_pair_mime: (a: number, b: number, c: number) => void;
    readonly iodocument_addImage: (a: number, b: number, c: number) => [number, number, number, number];
    readonly iodocument_blank: (a: number, b: number) => [number, number, number];
    readonly iodocument_image: (a: number, b: number, c: number) => number;
    readonly iodocument_kind: (a: number) => [number, number];
    readonly iodocument_model: (a: number) => [number, number];
    readonly iodocument_new: (a: number, b: number) => [number, number, number];
    readonly iodocument_save: (a: number, b: number) => [number, number, number, number];
    readonly iodocument_setModel: (a: number, b: number, c: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
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
