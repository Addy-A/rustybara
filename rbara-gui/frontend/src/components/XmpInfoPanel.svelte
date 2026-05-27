<script>
  import { useAppState } from '../lib/context.js'
  const app = useAppState()

  let xmp = $derived(app.fileXmp)
  let currentOp = $derived(app.actionToOp[app.activeAction] ?? null)

  function isCurrentOp(op) {
    if (!currentOp) return false
    return op === currentOp || op.startsWith(currentOp + '(')
  }

  function opLabel(op) {
    // Strip params for display: "resize(bleed_in=0.125)" → "resize"
    return op.replace(/\(.*\)$/, '').replace(/_/g, ' ')
  }

  function formatTs(ts) {
    if (!ts) return ''
    try {
      return new Date(ts).toLocaleString(undefined, {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
      })
    } catch {
      return ts
    }
  }

  // Abbreviate hash for display: "sha256:abc123def456..." → "sha256:abc123…"
  function shortHash(h) {
    if (!h) return ''
    const colon = h.indexOf(':')
    const prefix = colon >= 0 ? h.slice(0, colon + 1) : ''
    const hex = colon >= 0 ? h.slice(colon + 1) : h
    return `${prefix}${hex.slice(0, 8)}…`
  }
</script>

{#if xmp}
  <div class="xmp-panel">
    <div class="xmp-head">
      <span class="xmp-label">rbara v{xmp.version}</span>
      <span class="xmp-badges">
        {#if xmp.source_stale === true}
          <span class="badge stale" title="Source file has changed since this output was produced">⚠ stale</span>
        {/if}
        {#if xmp.parent_id}
          <span class="badge chain" title="Derived from a previously processed file (parentId: {xmp.parent_id})">⛓</span>
        {/if}
      </span>
    </div>

    {#if xmp.ops.length > 0}
      <div class="ops-row">
        {#each xmp.ops as op (op)}
          <span
            class="op-chip"
            class:current={isCurrentOp(op)}
            title={op}
          >{opLabel(op)}</span>
        {/each}
      </div>
    {:else}
      <div class="ops-empty">no ops recorded</div>
    {/if}

    <div class="xmp-foot">
      <span class="xmp-ts">{formatTs(xmp.timestamp)}</span>
      <span class="xmp-hash" title={xmp.source_hash}>{shortHash(xmp.source_hash)}</span>
    </div>
  </div>
{/if}

<style>
  .xmp-panel {
    border-top: 1px solid var(--border);
    padding: 8px 12px 6px;
    background: var(--surface);
    flex-shrink: 0;
  }
  .xmp-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 5px;
  }
  .xmp-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }
  .xmp-badges {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .badge {
    font-size: 9px;
    font-family: var(--mono);
    border-radius: 3px;
    padding: 1px 4px;
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
    font-size: 11px;
    padding: 0 3px;
  }
  .ops-row {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    margin-bottom: 5px;
  }
  .op-chip {
    font-size: 9.5px;
    font-family: var(--mono);
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 1px 5px;
    color: var(--muted-hi);
    white-space: nowrap;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .op-chip.current {
    background: var(--orange-dim);
    border-color: var(--orange);
    color: var(--orange-hi);
  }
  .ops-empty {
    font-size: 10px;
    color: var(--muted);
    font-style: italic;
    margin-bottom: 5px;
  }
  .xmp-foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .xmp-ts {
    font-size: 9.5px;
    color: var(--muted);
    font-family: var(--mono);
  }
  .xmp-hash {
    font-size: 9px;
    color: var(--muted);
    font-family: var(--mono);
    opacity: 0.6;
    cursor: default;
  }
</style>
