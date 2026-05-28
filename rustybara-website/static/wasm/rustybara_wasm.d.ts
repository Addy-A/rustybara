/* tslint:disable */
/* eslint-disable */

/**
 * In-browser PDF pipeline handle.
 */
export class PipelineHandle {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Set a TrimBox on every page by insetting the MediaBox by `bleed_pts` on all sides.
     * Use this when a PDF has no TrimBox but the bleed extent is known (typically 9 pts / ⅛ in).
     * Consumes the handle and returns a new one.
     */
    add_trim_box(bleed_pts: number): PipelineHandle;
    /**
     * Classify color usage across all pages.
     * Returns one of `"CMYK"`, `"RGB"`, `"Mixed"`, or `"Unknown"`.
     */
    detect_color_space(): string;
    /**
     * Embed rustybara processing metadata into the document's XMP stream.
     *
     * `source_hash` should be the result of calling `hash_bytes()` on the original
     * unmodified PDF bytes. `timestamp` is an ISO 8601 string from the caller.
     * `op_names` and `op_params` are parallel arrays of operation names and their
     * parameter strings (use an empty string for ops with no parameters).
     * Consumes the handle and returns a new one.
     */
    embed_metadata(source_hash: string, timestamp: string, op_names: string[], op_params: string[]): PipelineHandle;
    /**
     * Extract a subset of pages into a new handle. Page indices are zero-based.
     * Out-of-range indices are silently ignored. Does not consume this handle.
     */
    extract_pages(page_nums: Uint32Array): PipelineHandle;
    /**
     * Construct from raw PDF bytes.
     */
    constructor(bytes: Uint8Array);
    /**
     * Convert all text to outlined vector paths, removing the dependency on embedded fonts.
     * Consumes the handle and returns a new one.
     */
    outline_text(): PipelineHandle;
    /**
     * Return the number of pages in the document (does not consume the handle).
     */
    page_count(): number;
    /**
     * Return approximate text and image bounding boxes for a page as
     * `{ text: [[x,y,w,h], …], images: [[x,y,w,h], …] }` (pts, origin bottom-left).
     * `page_idx` is zero-based. Returns an object with empty arrays when the page
     * does not exist or has no parseable content.
     */
    page_layout_hint(page_idx: number): any;
    /**
     * Read the `rbara:` XMP block embedded by a previous rustybara run.
     * Returns `null` for files that have never been processed by rustybara,
     * otherwise an object with `uuid`, `version`, `timestamp`, `source_hash`,
     * `parent_id`, and `ops` (string array) fields.
     */
    read_xmp_block(): any;
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
     * Split each wide page at `panel_width_pts` into left/right halves.
     * Does not consume this handle; returns a new one containing the split pages.
     */
    split_pages(panel_width_pts: number): PipelineHandle;
    /**
     * Stitch adjacent page pairs into spreads of `spread_width_pts`.
     * Does not consume this handle; returns a new one containing the stitched spreads.
     */
    stitch_pages(spread_width_pts: number): PipelineHandle;
    /**
     * Serialize the result to PDF bytes for download. Consumes the handle.
     */
    to_pdf_bytes(): Uint8Array;
    /**
     * Strip content outside the TrimBox. Consumes the handle and returns a new one.
     */
    trim(): PipelineHandle;
}

/**
 * Compute `"sha256:<hex>"` of raw bytes. Call this on the original PDF bytes
 * *before* constructing a `PipelineHandle` so the hash reflects unmodified data.
 */
export function hash_bytes(bytes: Uint8Array): string;

export function init(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_pipelinehandle_free: (a: number, b: number) => void;
    readonly hash_bytes: (a: number, b: number) => [number, number];
    readonly pipelinehandle_add_trim_box: (a: number, b: number) => [number, number, number];
    readonly pipelinehandle_detect_color_space: (a: number) => [number, number];
    readonly pipelinehandle_embed_metadata: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number, number];
    readonly pipelinehandle_extract_pages: (a: number, b: number, c: number) => [number, number, number];
    readonly pipelinehandle_new: (a: number, b: number) => [number, number, number];
    readonly pipelinehandle_outline_text: (a: number) => [number, number, number];
    readonly pipelinehandle_page_count: (a: number) => number;
    readonly pipelinehandle_page_layout_hint: (a: number, b: number) => any;
    readonly pipelinehandle_read_xmp_block: (a: number) => any;
    readonly pipelinehandle_remap_color: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => [number, number, number];
    readonly pipelinehandle_resize: (a: number, b: number) => [number, number, number];
    readonly pipelinehandle_split_pages: (a: number, b: number) => [number, number, number];
    readonly pipelinehandle_stitch_pages: (a: number, b: number) => [number, number, number];
    readonly pipelinehandle_to_pdf_bytes: (a: number) => [number, number, number, number];
    readonly pipelinehandle_trim: (a: number) => [number, number, number];
    readonly init: () => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
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
