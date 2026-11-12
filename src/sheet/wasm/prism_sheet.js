/* @ts-self-types="./prism_sheet.d.ts" */

export class SheetEngine {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SheetEngineFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_sheetengine_free(ptr, 0);
    }
    /**
     * @param {number} sheet
     * @param {string} def_json
     * @returns {number}
     */
    addChart(sheet, def_json) {
        const val2 = +sheet;
        const ptr0 = passStringToWasm0(def_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sheetengine_addChart(this.__wbg_ptr, val2, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Add a classic PivotTable (`PivotDef` JSON) and fill it.
     * @param {number} sheet
     * @param {string} def_json
     * @param {number} now
     * @returns {string}
     */
    addPivot(sheet, def_json, now) {
        let deferred5_0;
        let deferred5_1;
        try {
            const val2 = +sheet;
            const val3 = +now;
            const ptr0 = passStringToWasm0(def_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.sheetengine_addPivot(this.__wbg_ptr, val2, ptr0, len0, val3);
            var ptr4 = ret[0];
            var len4 = ret[1];
            if (ret[3]) {
                ptr4 = 0; len4 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred5_0 = ptr4;
            deferred5_1 = len4;
            return getStringFromWasm0(ptr4, len4);
        } finally {
            wasm.__wbindgen_free(deferred5_0, deferred5_1, 1);
        }
    }
    /**
     * @param {string} name
     * @returns {number}
     */
    addSheet(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sheetengine_addSheet(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * @param {number} sheet
     * @param {number} r0
     * @param {number} c0
     * @param {number} r1
     * @param {number} c1
     * @param {string | null | undefined} num_format
     * @param {string} patch_json
     */
    applyStyle(sheet, r0, c0, r1, c1, num_format, patch_json) {
        const val3 = +sheet;
        const val4 = +r0;
        const val5 = +c0;
        const val6 = +r1;
        const val7 = +c1;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = isLikeNone(num_format) ? 0 : passStringToWasm0(num_format, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(patch_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.sheetengine_applyStyle(this.__wbg_ptr, val3, val4, val5, val6, val7, ptr0, len0, ptr1, len1);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Pending `=PY` requests.
     * @returns {string}
     */
    asyncRequests() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_asyncRequests(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * `[{chart, data}]` for a sheet.
     * @param {number} sheet
     * @returns {string}
     */
    charts(sheet) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_charts(this.__wbg_ptr, sheet);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    connections() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_connections(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} sheet
     * @param {number} now
     * @returns {string}
     */
    deleteSheet(sheet, now) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.sheetengine_deleteSheet(this.__wbg_ptr, sheet, now);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * `[{sheet,r,c,text}]` → change report.
     * @param {string} edits_json
     * @param {number} now
     * @returns {string}
     */
    edit(edits_json, now) {
        let deferred4_0;
        let deferred4_1;
        try {
            const val2 = +now;
            const ptr0 = passStringToWasm0(edits_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.sheetengine_edit(this.__wbg_ptr, ptr0, len0, val2);
            var ptr3 = ret[0];
            var len3 = ret[1];
            if (ret[3]) {
                ptr3 = 0; len3 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred4_0 = ptr3;
            deferred4_1 = len3;
            return getStringFromWasm0(ptr3, len3);
        } finally {
            wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
        }
    }
    /**
     * @param {number} sheet
     * @param {number} sr0
     * @param {number} sc0
     * @param {number} sr1
     * @param {number} sc1
     * @param {number} dr0
     * @param {number} dc0
     * @param {number} dr1
     * @param {number} dc1
     * @param {number} now
     * @returns {string}
     */
    fill(sheet, sr0, sc0, sr1, sc1, dr0, dc0, dr1, dc1, now) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_fill(this.__wbg_ptr, sheet, sr0, sc0, sr1, sc1, dr0, dc0, dr1, dc1, now);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    static functionNames() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_functionNames();
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} sheet
     * @param {number} r
     * @param {number} c
     * @returns {string}
     */
    input(sheet, r, c) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_input(this.__wbg_ptr, sheet, r, c);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    model() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_model(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Load a `WorkbookModel` (JSON) and calculate it. `now` is an Excel serial.
     * @param {string} model_json
     * @param {number} now
     */
    constructor(model_json, now) {
        const val2 = +now;
        const ptr0 = passStringToWasm0(model_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sheetengine_new(ptr0, len0, val2);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        SheetEngineFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @param {number} src_sheet
     * @param {number} sr0
     * @param {number} sc0
     * @param {number} sr1
     * @param {number} sc1
     * @param {number} dst_sheet
     * @param {number} dr
     * @param {number} dc
     * @param {number} now
     * @returns {string}
     */
    paste(src_sheet, sr0, sc0, sr1, sc1, dst_sheet, dr, dc, now) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_paste(this.__wbg_ptr, src_sheet, sr0, sc0, sr1, sc1, dst_sheet, dr, dc, now);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * PivotTables of a sheet.
     * @param {number} sheet
     * @returns {string}
     */
    pivots(sheet) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_pivots(this.__wbg_ptr, sheet);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} now
     * @returns {string}
     */
    recalc(now) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_recalc(this.__wbg_ptr, now);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} now
     * @returns {string}
     */
    refreshPivots(now) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.sheetengine_refreshPivots(this.__wbg_ptr, now);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @param {number} sheet
     * @param {number} index
     */
    removeChart(sheet, index) {
        wasm.sheetengine_removeChart(this.__wbg_ptr, sheet, index);
    }
    /**
     * @param {number} sheet
     * @param {number} index
     * @param {number} now
     * @returns {string}
     */
    removePivot(sheet, index, now) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_removePivot(this.__wbg_ptr, sheet, index, now);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} sheet
     * @param {string} name
     * @param {number} now
     * @returns {string}
     */
    renameSheet(sheet, name, now) {
        let deferred5_0;
        let deferred5_1;
        try {
            const val2 = +sheet;
            const val3 = +now;
            const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.sheetengine_renameSheet(this.__wbg_ptr, val2, ptr0, len0, val3);
            var ptr4 = ret[0];
            var len4 = ret[1];
            if (ret[3]) {
                ptr4 = 0; len4 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred5_0 = ptr4;
            deferred5_1 = len4;
            return getStringFromWasm0(ptr4, len4);
        } finally {
            wasm.__wbindgen_free(deferred5_0, deferred5_1, 1);
        }
    }
    /**
     * Negative width resets to the default.
     * @param {number} sheet
     * @param {number} col
     * @param {number} width
     */
    setColWidth(sheet, col, width) {
        wasm.sheetengine_setColWidth(this.__wbg_ptr, sheet, col, width);
    }
    /**
     * @param {string} json_text
     */
    setConnections(json_text) {
        const ptr0 = passStringToWasm0(json_text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sheetengine_setConnections(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} sheet
     * @param {number} rows
     * @param {number} cols
     */
    setFrozen(sheet, rows, cols) {
        wasm.sheetengine_setFrozen(this.__wbg_ptr, sheet, rows, cols);
    }
    /**
     * @param {number} sheet
     * @param {number} row
     * @param {number} height
     */
    setRowHeight(sheet, row, height) {
        wasm.sheetengine_setRowHeight(this.__wbg_ptr, sheet, row, height);
    }
    /**
     * @returns {string}
     */
    sheets() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_sheets(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} sheet
     * @param {number} r
     * @param {number} c
     * @returns {string}
     */
    styleOf(sheet, r, c) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_styleOf(this.__wbg_ptr, sheet, r, c);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Visual styles by style index.
     * @returns {string}
     */
    styles() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_styles(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Answer a `=PY` request with a value (`1`, `"x"`, `true`, `{"e":"#VALUE!"}`) or a 2-D array.
     * @param {string} key
     * @param {number} sheet
     * @param {number} r
     * @param {number} c
     * @param {string} value_json
     * @param {number} now
     * @returns {string}
     */
    supplyAsync(key, sheet, r, c, value_json, now) {
        let deferred8_0;
        let deferred8_1;
        try {
            const val3 = +sheet;
            const val4 = +r;
            const val5 = +c;
            const val6 = +now;
            let ptr0, len0, ptr1, len1;
            try {
                ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len0 = WASM_VECTOR_LEN;
                ptr1 = passStringToWasm0(value_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len1 = WASM_VECTOR_LEN;
            } catch (e) {
                if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                throw e;
            }
            const ret = wasm.sheetengine_supplyAsync(this.__wbg_ptr, ptr0, len0, val3, val4, val5, ptr1, len1, val6);
            var ptr7 = ret[0];
            var len7 = ret[1];
            if (ret[3]) {
                ptr7 = 0; len7 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred8_0 = ptr7;
            deferred8_1 = len7;
            return getStringFromWasm0(ptr7, len7);
        } finally {
            wasm.__wbindgen_free(deferred8_0, deferred8_1, 1);
        }
    }
    /**
     * @param {number} sheet
     * @param {number} index
     * @param {string} def_json
     */
    updateChart(sheet, index, def_json) {
        const val2 = +sheet;
        const val3 = +index;
        const ptr0 = passStringToWasm0(def_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sheetengine_updateChart(this.__wbg_ptr, val2, val3, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} sheet
     * @param {number} r0
     * @param {number} c0
     * @param {number} r1
     * @param {number} c1
     * @returns {string}
     */
    view(sheet, r0, c0, r1, c1) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sheetengine_view(this.__wbg_ptr, sheet, r0, c0, r1, c1);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) SheetEngine.prototype[Symbol.dispose] = SheetEngine.prototype.free;

/**
 * Timing breakdown of loading a model (diagnostics).
 * @param {string} model_json
 * @returns {string}
 */
export function profileLoad(model_json) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ptr0 = passStringToWasm0(model_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.profileLoad(ptr0, len0);
        deferred2_0 = ret[0];
        deferred2_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Error_208446807d0d1e30: function(arg0, arg1) {
            const ret = Error(getStringFromWasm0(arg0, arg1));
            return ret;
        },
        __wbg___wbindgen_throw_4c88d8bf264fd984: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_now_b66ab5cb701a7095: function() {
            const ret = Date.now();
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            table.set(0, undefined);
            table.set(1024, undefined);
            table.set(1025, null);
            table.set(1026, true);
            table.set(1027, false);
        },
    };
    return {
        __proto__: null,
        "./prism_sheet_bg.js": import0,
    };
}

const SheetEngineFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_sheetengine_free(ptr, 1));

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(bytes) {
    numBytesDecoded += bytes.length;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
        cachedTextDecoder.decode();
        numBytesDecoded = bytes.length;
    }
    return cachedTextDecoder.decode(bytes);
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

let __wbg_init_promise;
async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;
    if (__wbg_init_promise === undefined) {
        __wbg_init_promise = __wbg_init_once(module_or_path);
        __wbg_init_promise.catch(() => { __wbg_init_promise = undefined; });
    }
    return __wbg_init_promise;
}

async function __wbg_init_once(module_or_path) {

    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('prism_sheet_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
