// Resource-gating logic for large files.
//
// rustybara cannot yet parse very large PDFs without freezing (see README "Known
// Limitations"), so oversized files are hard-blocked on add rather than processed.
// This module holds the *pure* size decision so it can be unit tested in isolation
// from Svelte/Tauri.

/**
 * Decide whether a file of `sizeKb` should be blocked from being added, given the
 * configured size limit `limitMb`.
 *
 * Rules, in order:
 *  1. Unknown / invalid size (null, 0, NaN, non-finite, non-number) → `true`
 *     (fail safe — never let a file of unknown size through the gate).
 *  2. Limit disabled (`limitMb` <= 0 or non-numeric) → `false` (no block). The
 *     user has opted out; large files may hang the app (the UI warns about this).
 *  3. Otherwise → `true` when the file exceeds the limit.
 *
 * @param {number|null|undefined} sizeKb  File size in kilobytes (from fs metadata).
 * @param {number|null|undefined} limitMb Block threshold in MB. `0`/negative/missing
 *        disables the block.
 * @returns {boolean} true if the file should be refused.
 */
export function sizeExceedsLimit(sizeKb, limitMb) {
  if (!sizeKb || typeof sizeKb !== 'number' || !isFinite(sizeKb)) return true

  const limit = typeof limitMb === 'number' && isFinite(limitMb) ? limitMb : 0
  if (limit <= 0) return false

  return sizeKb / 1024 > limit
}
