<script>
  import { useAppState } from '../lib/context.js'
  const app = useAppState()

  // ---- section collapse state ----
  let logOpen = $state(true)
  let histOpen = $state(true)

  // ---- resize ----
  const MIN_W = 200
  const MAX_W = 480
  const STORE_KEY = 'rbara-log-pane-width'

  let width = $state(Math.max(MIN_W, Math.min(MAX_W,
    parseInt(localStorage.getItem(STORE_KEY) ?? '255', 10)
  )))

  let dragging = false
  let startX = 0
  let startWidth = 0

  function onHandleDown(e) {
    dragging = true
    startX = e.clientX
    startWidth = width
    e.preventDefault()
  }

  function onMouseMove(e) {
    if (!dragging) return
    // Handle is on the left edge — dragging left widens, right narrows.
    width = Math.max(MIN_W, Math.min(MAX_W, startWidth - (e.clientX - startX)))
    localStorage.setItem(STORE_KEY, width)
  }

  function onMouseUp() {
    dragging = false
  }

  // ---- activity log helpers ----
  function badgeClass(entry) { return entry.ok ? 'log-ok' : 'log-fail' }
  function badgeText(entry)  { return entry.ok ? 'OK' : 'FAIL' }

  // ---- XMP history helpers ----

  // Each op string is "name@ISO8601" or "name(params)@ISO8601".
  // Old entries without "@" are handled gracefully (ts: null).
  function parseOp(opStr) {
    const at = opStr.lastIndexOf('@')
    if (at > 0) {
      return { body: opStr.slice(0, at), ts: opStr.slice(at + 1) }
    }
    return { body: opStr, ts: null }
  }

  function opName(opStr) {
    // "resize(bleed_in=0.125)" → "resize"
    return parseOp(opStr).body.replace(/\(.*\)$/, '').replace(/_/g, ' ')
  }

  function opParams(opStr) {
    const m = parseOp(opStr).body.match(/\((.+)\)$/)
    return m ? m[1] : null
  }

  function opDate(opStr) {
    const { ts } = parseOp(opStr)
    if (!ts) return null
    try {
      const d = new Date(ts)
      const mm = String(d.getUTCMonth() + 1).padStart(2, '0')
      const dd = String(d.getUTCDate()).padStart(2, '0')
      const yyyy = d.getUTCFullYear()
      return `${mm}/${dd}/${yyyy}`
    } catch { return null }
  }

  function opTimeUtc(opStr) {
    const { ts } = parseOp(opStr)
    if (!ts) return null
    try {
      return new Date(ts).toLocaleTimeString('en-US', {
        hour: '2-digit', minute: '2-digit', timeZone: 'UTC', timeZoneName: 'short',
      })
    } catch { return null }
  }

  let groupedOps = $derived.by(() => {
    const ops = app.fileXmp?.ops
    if (!ops?.length) return []
    const groups = []
    const index = new Map()
    for (const op of ops) {
      const date = opDate(op) ?? 'Unknown date'
      if (!index.has(date)) {
        index.set(date, groups.length)
        groups.push({ date, ops: [] })
      }
      groups[index.get(date)].ops.push(op)
    }
    return groups
  })

  function isCurrentOp(opStr) {
    const currentName = app.actionToOp[app.activeAction] ?? null
    if (!currentName) return false
    const body = parseOp(opStr).body
    return body === currentName || body.startsWith(currentName + '(')
  }

  function shortHash(h) {
    if (!h) return ''
    const colon = h.indexOf(':')
    const prefix = colon >= 0 ? h.slice(0, colon + 1) : ''
    const hex    = colon >= 0 ? h.slice(colon + 1) : h
    return `${prefix}${hex.slice(0, 8)}…`
  }
</script>

<svelte:window onmousemove={onMouseMove} onmouseup={onMouseUp} />

<div class="log-pane" style="width: {width}px">
  <div
    class="resize-handle"
    onmousedown={onHandleDown}
    role="separator"
    aria-orientation="vertical"
  ></div>

  <!-- ── Activity Log ── -->
  <div
    class="section-header"
    onclick={() => (logOpen = !logOpen)}
    role="button"
    tabindex="0"
  >
    <span class="section-label">Activity Log</span>
    <span class="chevron">{logOpen ? '▾' : '▸'}</span>
  </div>

  {#if logOpen}
    <div class="log-list">
      {#each app.actionLog as entry, i (i)}
        <div class="log-entry">
          <div class="log-header-row">
            <span class="log-badge {badgeClass(entry)}">{badgeText(entry)}</span>
            <span class="log-action">{entry.action}</span>
            <span class="log-time">{entry.timestamp}</span>
          </div>
          <div class="log-detail" class:err={!entry.ok}>{entry.message}</div>
        </div>
      {:else}
        <div class="log-empty">No activity yet</div>
      {/each}
    </div>
  {/if}

  <!-- ── File History ── -->
  <div
    class="section-header"
    onclick={() => (histOpen = !histOpen)}
    role="button"
    tabindex="0"
  >
    <span class="section-label">File History</span>
    <span class="chevron">{histOpen ? '▾' : '▸'}</span>
  </div>

  {#if histOpen}
    {#if app.fileXmp}
      <!-- metadata strip -->
      <div class="hist-meta-strip">
        <span class="hist-version">rbara v{app.fileXmp.version}</span>
        <span class="hist-ids">
          <span class="hist-hash" title={app.fileXmp.source_hash}>{shortHash(app.fileXmp.source_hash)}</span>
          <span class="hist-uuid" title="Document UUID: {app.fileXmp.uuid}">id:{app.fileXmp.uuid.slice(0, 8)}…</span>
        </span>
        {#if app.fileXmp.source_stale === true}
          <span class="badge stale" title="Source file has changed since this output was produced">⚠ stale</span>
        {/if}
        {#if app.fileXmp.parent_id}
          <span class="badge chain" title="Derived from a previously processed file&#10;parentId: {app.fileXmp.parent_id}">⛓ derived</span>
        {/if}
      </div>

      <!-- op records grouped by date -->
      {#if groupedOps.length > 0}
        {#each groupedOps as group (group.date)}
          <div class="hist-date-header">{group.date}</div>
          {#each group.ops as op (op)}
            <div class="hist-entry" class:current={isCurrentOp(op)}>
              <div class="hist-header-row">
                <span class="hist-badge" class:current={isCurrentOp(op)}>OP</span>
                <span class="hist-op-name">{opName(op)}</span>
                {#if opTimeUtc(op)}
                  <span class="hist-time">{opTimeUtc(op)}</span>
                {/if}
              </div>
              {#if opParams(op)}
                <div class="hist-detail">{opParams(op)}</div>
              {/if}
            </div>
          {/each}
        {/each}
      {:else}
        <div class="log-empty">No ops recorded</div>
      {/if}
    {:else}
      <div class="log-empty">No rbara metadata in selected file</div>
    {/if}
  {/if}

  <!-- ── pinned footer ── -->
  <div class="output-row">
    <div class="output-row-label">Output Directory</div>
    <div class="output-path-value">
      {app.overwrite ? '(overwriting source)' : (app.outputDir ?? '~/source folder')}
    </div>
  </div>
</div>

<style>
  .log-pane {
    flex-shrink: 0;
    background: var(--surface);
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    position: relative;
  }

  /* ── resize handle ── */
  .resize-handle {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 4px;
    cursor: ew-resize;
    z-index: 10;
    transition: background 0.15s;
  }
  .resize-handle:hover {
    background: color-mix(in srgb, var(--orange) 40%, transparent);
  }

  /* ── section headers ── */
  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--muted);
    border-bottom: 1px solid var(--border);
    cursor: pointer;
    user-select: none;
    flex-shrink: 0;
    transition: color 0.1s, background 0.1s;
  }
  .section-header:hover {
    color: var(--muted-hi);
    background: var(--panel);
  }
  .section-label { flex: 1 }
  .chevron {
    font-size: 10px;
    color: var(--muted);
  }

  /* ── activity log ── */
  .log-list {
    flex: 1;
    overflow-y: auto;
    min-height: 60px;
  }
  .log-entry {
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    font-family: var(--mono);
    font-size: 11px;
  }
  .log-entry:hover { background: var(--panel) }
  .log-header-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 3px;
  }
  .log-badge {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 3px;
    letter-spacing: 0.06em;
  }
  .log-ok {
    background: color-mix(in srgb, var(--ok) 12%, transparent);
    color: var(--ok);
    border: 1px solid color-mix(in srgb, var(--ok) 40%, transparent);
  }
  .log-fail {
    background: color-mix(in srgb, var(--fail) 12%, transparent);
    color: var(--fail);
    border: 1px solid color-mix(in srgb, var(--fail) 40%, transparent);
  }
  .log-time {
    font-size: 10px;
    color: var(--muted);
    margin-left: auto;
  }
  .log-action { color: var(--text) }
  .log-detail {
    font-size: 10.5px;
    color: var(--muted);
    line-height: 1.4;
  }
  .log-detail.err { color: var(--fail) }
  .log-empty {
    padding: 14px 10px;
    color: var(--muted);
    font-size: 11px;
    text-align: center;
    font-style: italic;
  }

  /* ── file history ── */
  .hist-meta-strip {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }
  .hist-version {
    font-size: 10px;
    font-weight: 600;
    font-family: var(--mono);
    color: var(--muted-hi);
  }
  .hist-ids {
    display: flex;
    gap: 5px;
    align-items: center;
    margin-left: auto;
  }
  .hist-hash {
    font-size: 9px;
    font-family: var(--mono);
    color: var(--muted);
    opacity: 0.6;
    cursor: default;
  }
  .hist-uuid {
    font-size: 9px;
    font-family: var(--mono);
    color: var(--muted);
    opacity: 0.5;
    cursor: default;
  }
  .badge {
    font-size: 9px;
    font-family: var(--mono);
    border-radius: 3px;
    padding: 1px 5px;
    line-height: 1.4;
  }
  .badge.stale {
    background: rgba(220, 80, 40, 0.15);
    color: #e06040;
    border: 1px solid rgba(220, 80, 40, 0.3);
  }
  .badge.chain {
    background: var(--panel);
    color: var(--muted-hi);
    border: 1px solid var(--border);
  }

  /* date group headers */
  .hist-date-header {
    padding: 4px 10px;
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    z-index: 1;
  }

  /* op records — mirror activity log entry style */
  .hist-entry {
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    font-family: var(--mono);
    font-size: 11px;
  }
  .hist-entry:hover { background: var(--panel) }
  .hist-entry.current {
    background: color-mix(in srgb, var(--orange-dim) 60%, transparent);
  }
  .hist-header-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 3px;
  }
  .hist-badge {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 3px;
    letter-spacing: 0.06em;
    background: var(--panel);
    color: var(--muted-hi);
    border: 1px solid var(--border);
  }
  .hist-badge.current {
    background: color-mix(in srgb, var(--orange) 15%, transparent);
    color: var(--orange-hi);
    border-color: color-mix(in srgb, var(--orange) 50%, transparent);
  }
  .hist-op-name { color: var(--text) }
  .hist-time {
    font-size: 10px;
    color: var(--muted);
    margin-left: auto;
  }
  .hist-detail {
    font-size: 10.5px;
    color: var(--muted);
    line-height: 1.4;
  }

  /* ── output footer ── */
  .output-row {
    padding: 8px 10px;
    border-top: 1px solid var(--border);
    font-size: 11px;
    font-family: var(--mono);
    color: var(--muted);
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-top: auto;
    flex-shrink: 0;
  }
  .output-row-label {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--muted);
  }
  .output-path-value {
    color: var(--muted-hi);
    word-break: break-all;
    line-height: 1.4;
  }
</style>
