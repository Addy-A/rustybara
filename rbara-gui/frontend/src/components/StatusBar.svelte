<script>
  import { useAppState } from '../lib/context.js'
  const app = useAppState()

  let f = $derived(app.activeFileObj)

  const labels = {
    trim: 'Trim Marks',
    resize: 'Resize to Bleed',
    export: 'Export Images',
    remap: 'Remap Colors',
    output: 'Output Path',
  }
</script>

<div class="statusbar">
  <div class="sb-item">
    <div
      class="sb-dot"
      style="background: {app.files.length ? 'var(--ok)' : 'var(--muted)'}"
    ></div>
    {app.scopedCount}/{app.files.length} scoped
  </div>
  {#if f}
    <div class="sb-item">{f.name}</div>
    <div class="sb-item" style="color: #f97316">
      {app.metadata?.color_space ?? '—'}
    </div>
  {:else}
    <div class="sb-item idle">Last Action: {app.quip}</div>
  {/if}
  <div class="sb-right">
    {#if app.processing}
      <div class="sb-item processing">⏳ Processing…</div>
    {/if}
    <div class="sb-item action">{labels[app.activeAction]} selected</div>
    <div class="sb-item">↑/↓ navigate · Enter run · ? help</div>
  </div>
</div>

<style>
  .statusbar {
    height: 24px;
    background: var(--surface);
    border-top: 1px solid var(--border);
    display: flex;
    align-items: center;
    padding: 0 12px;
    gap: 16px;
    flex-shrink: 0;
    font-size: 11px;
    color: var(--muted);
    font-family: var(--mono);
  }
  .sb-item {
    display: flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sb-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }
  .sb-right {
    margin-left: auto;
    display: flex;
    gap: 16px;
  }
  .sb-item.action {
    color: var(--orange);
  }
  .sb-item.idle {
    color: var(--orange);
    font-style: italic;
  }
  .sb-item.processing {
    color: var(--warn);
  }
</style>
