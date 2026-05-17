/* tslint:disable */
/* eslint-disable */

/**
 * In-browser PDF pipeline handle.
 */
export class PipelineHandle {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Construct from raw PDF bytes.
     */
    constructor(bytes: Uint8Array);
    /**
     * Return the number of pages in the document (does not consume the handle).
     */
    page_count(): number;
    /**
     * Substitute a CMYK color throughout content streams.
     * All channel values are in the 0.0–1.0 range. Consumes the handle and returns a new one.
     */
    remap_color(from_c: number, from_m: number, from_y: number, from_k: number, to_c: number, to_m: number, to_y: number, to_k: number, tolerance: number): PipelineHandle;
    /**
     * Expand all page boxes by `bleed_pts` PDF points. Consumes the handle and returns a new one.
     */
    resize(bleed_pts: number): PipelineHandle;
    /**
     * Serialize the result to PDF bytes for download. Consumes the handle.
     */
    to_pdf_bytes(): Uint8Array;
    /**
     * Strip content outside the TrimBox. Consumes the handle and returns a new one.
     */
    trim(): PipelineHandle;
}

export function init(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_pipelinehandle_free: (a: number, b: number) => void;
    readonly pipelinehandle_new: (a: number, b: number) => [number, number, number];
    readonly pipelinehandle_page_count: (a: number) => number;
    readonly pipelinehandle_remap_color: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => [number, number, number];
    readonly pipelinehandle_resize: (a: number, b: number) => [number, number, number];
    readonly pipelinehandle_to_pdf_bytes: (a: number) => [number, number, number, number];
    readonly pipelinehandle_trim: (a: number) => [number, number, number];
    readonly init: () => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
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
