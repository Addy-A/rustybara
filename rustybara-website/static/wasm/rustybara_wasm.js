/* @ts-self-types="./rustybara_wasm.d.ts" */

/**
 * In-browser PDF pipeline handle.
 */
export class PipelineHandle {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(PipelineHandle.prototype);
        obj.__wbg_ptr = ptr;
        PipelineHandleFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        PipelineHandleFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_pipelinehandle_free(ptr, 0);
    }
    /**
     * Set a TrimBox on every page by insetting the MediaBox by `bleed_pts` on all sides.
     * Use this when a PDF has no TrimBox but the bleed extent is known (typically 9 pts / ⅛ in).
     * Consumes the handle and returns a new one.
     * @param {number} bleed_pts
     * @returns {PipelineHandle}
     */
    add_trim_box(bleed_pts) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.pipelinehandle_add_trim_box(ptr, bleed_pts);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return PipelineHandle.__wrap(ret[0]);
    }
    /**
     * Classify color usage across all pages.
     * Returns one of `"CMYK"`, `"RGB"`, `"Mixed"`, or `"Unknown"`.
     * @returns {string}
     */
    detect_color_space() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.pipelinehandle_detect_color_space(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Embed rustybara processing metadata into the document's XMP stream.
     *
     * `source_hash` should be the result of calling `hash_bytes()` on the original
     * unmodified PDF bytes. `timestamp` is an ISO 8601 string from the caller.
     * `op_names` and `op_params` are parallel arrays of operation names and their
     * parameter strings (use an empty string for ops with no parameters).
     * Consumes the handle and returns a new one.
     * @param {string} source_hash
     * @param {string} timestamp
     * @param {string[]} op_names
     * @param {string[]} op_params
     * @returns {PipelineHandle}
     */
    embed_metadata(source_hash, timestamp, op_names, op_params) {
        const ptr = this.__destroy_into_raw();
        const ptr0 = passStringToWasm0(source_hash, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(timestamp, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayJsValueToWasm0(op_names, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArrayJsValueToWasm0(op_params, wasm.__wbindgen_malloc);
        const len3 = WASM_VECTOR_LEN;
        const ret = wasm.pipelinehandle_embed_metadata(ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return PipelineHandle.__wrap(ret[0]);
    }
    /**
     * Extract a subset of pages into a new handle. Page indices are zero-based.
     * Out-of-range indices are silently ignored. Does not consume this handle.
     * @param {Uint32Array} page_nums
     * @returns {PipelineHandle}
     */
    extract_pages(page_nums) {
        const ptr0 = passArray32ToWasm0(page_nums, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.pipelinehandle_extract_pages(this.__wbg_ptr, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return PipelineHandle.__wrap(ret[0]);
    }
    /**
     * Construct from raw PDF bytes.
     * @param {Uint8Array} bytes
     */
    constructor(bytes) {
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.pipelinehandle_new(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        PipelineHandleFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Convert all text to outlined vector paths, removing the dependency on embedded fonts.
     * Consumes the handle and returns a new one.
     * @returns {PipelineHandle}
     */
    outline_text() {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.pipelinehandle_outline_text(ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return PipelineHandle.__wrap(ret[0]);
    }
    /**
     * Return the number of pages in the document (does not consume the handle).
     * @returns {number}
     */
    page_count() {
        const ret = wasm.pipelinehandle_page_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Return approximate text and image bounding boxes for a page as
     * `{ text: [[x,y,w,h], …], images: [[x,y,w,h], …] }` (pts, origin bottom-left).
     * `page_idx` is zero-based. Returns an object with empty arrays when the page
     * does not exist or has no parseable content.
     * @param {number} page_idx
     * @returns {any}
     */
    page_layout_hint(page_idx) {
        const ret = wasm.pipelinehandle_page_layout_hint(this.__wbg_ptr, page_idx);
        return ret;
    }
    /**
     * Read the `rbara:` XMP block embedded by a previous rustybara run.
     * Returns `null` for files that have never been processed by rustybara,
     * otherwise an object with `uuid`, `version`, `timestamp`, `source_hash`,
     * `parent_id`, and `ops` (string array) fields.
     * @returns {any}
     */
    read_xmp_block() {
        const ret = wasm.pipelinehandle_read_xmp_block(this.__wbg_ptr);
        return ret;
    }
    /**
     * Substitute a CMYK color throughout content streams.
     * All channel values are in the 0.0–1.0 range. Consumes the handle and returns a new one.
     * @param {number} from_c
     * @param {number} from_m
     * @param {number} from_y
     * @param {number} from_k
     * @param {number} to_c
     * @param {number} to_m
     * @param {number} to_y
     * @param {number} to_k
     * @param {number} tolerance
     * @returns {PipelineHandle}
     */
    remap_color(from_c, from_m, from_y, from_k, to_c, to_m, to_y, to_k, tolerance) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.pipelinehandle_remap_color(ptr, from_c, from_m, from_y, from_k, to_c, to_m, to_y, to_k, tolerance);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return PipelineHandle.__wrap(ret[0]);
    }
    /**
     * Expand all page boxes by `bleed_pts` PDF points. Consumes the handle and returns a new one.
     * @param {number} bleed_pts
     * @returns {PipelineHandle}
     */
    resize(bleed_pts) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.pipelinehandle_resize(ptr, bleed_pts);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return PipelineHandle.__wrap(ret[0]);
    }
    /**
     * Split each wide page at `panel_width_pts` into left/right halves.
     * Does not consume this handle; returns a new one containing the split pages.
     * @param {number} panel_width_pts
     * @returns {PipelineHandle}
     */
    split_pages(panel_width_pts) {
        const ret = wasm.pipelinehandle_split_pages(this.__wbg_ptr, panel_width_pts);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return PipelineHandle.__wrap(ret[0]);
    }
    /**
     * Stitch adjacent page pairs into spreads of `spread_width_pts`.
     * Does not consume this handle; returns a new one containing the stitched spreads.
     * @param {number} spread_width_pts
     * @returns {PipelineHandle}
     */
    stitch_pages(spread_width_pts) {
        const ret = wasm.pipelinehandle_stitch_pages(this.__wbg_ptr, spread_width_pts);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return PipelineHandle.__wrap(ret[0]);
    }
    /**
     * Serialize the result to PDF bytes for download. Consumes the handle.
     * @returns {Uint8Array}
     */
    to_pdf_bytes() {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.pipelinehandle_to_pdf_bytes(ptr);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Strip content outside the TrimBox. Consumes the handle and returns a new one.
     * @returns {PipelineHandle}
     */
    trim() {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.pipelinehandle_trim(ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return PipelineHandle.__wrap(ret[0]);
    }
}
if (Symbol.dispose) PipelineHandle.prototype[Symbol.dispose] = PipelineHandle.prototype.free;

/**
 * Compute `"sha256:<hex>"` of raw bytes. Call this on the original PDF bytes
 * *before* constructing a `PipelineHandle` so the hash reflects unmodified data.
 * @param {Uint8Array} bytes
 * @returns {string}
 */
export function hash_bytes(bytes) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.hash_bytes(ptr0, len0);
        deferred2_0 = ret[0];
        deferred2_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}

export function init() {
    wasm.init();
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_string_get_7ed5322991caaec5: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_6b64449b9b9ed33c: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_error_a6fa202b58aa1cd3: function(arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
            }
        },
        __wbg_getRandomValues_76dfc69825c9c552: function() { return handleError(function (arg0, arg1) {
            globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
        }, arguments); },
        __wbg_getRandomValues_ef12552bf5acd2fe: function() { return handleError(function (arg0, arg1) {
            globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
        }, arguments); },
        __wbg_new_227d7c05414eb861: function() {
            const ret = new Error();
            return ret;
        },
        __wbg_new_5e360d2ff7b9e1c3: function(arg0, arg1) {
            const ret = new Error(getStringFromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_new_682678e2f47e32bc: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_new_aa8d0fa9762c29bd: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_set_3bf1de9fab0cd644: function(arg0, arg1, arg2) {
            arg0[arg1 >>> 0] = arg2;
        },
        __wbg_set_6be42768c690e380: function(arg0, arg1, arg2) {
            arg0[arg1] = arg2;
        },
        __wbg_stack_3b0d974bbf31e44f: function(arg0, arg1) {
            const ret = arg1.stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbindgen_cast_0000000000000001: function(arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        },
        __wbindgen_cast_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./rustybara_wasm_bg.js": import0,
    };
}

const PipelineHandleFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_pipelinehandle_free(ptr >>> 0, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArray32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getUint32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayJsValueToWasm0(array, malloc) {
    const ptr = malloc(array.length * 4, 4) >>> 0;
    for (let i = 0; i < array.length; i++) {
        const add = addToExternrefTable0(array[i]);
        getDataViewMemory0().setUint32(ptr + 4 * i, add, true);
    }
    WASM_VECTOR_LEN = array.length;
    return ptr;
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

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
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

let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

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

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('rustybara_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
