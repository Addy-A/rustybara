<script>
  import { useAppState } from '../lib/context.js';
  import { formatSize } from '../lib/api.js';
  const app = useAppState();

  let { compact = false } = $props();

  function fmtBox(b) {
    if (!b) return '—';
    const w = ((b[2] - b[0]) / 72).toFixed(2);
    const h = ((b[3] - b[1]) / 72).toFixed(2);
    return `${w} × ${h} in`;
  }
  function fmtBleed(pts) {
    if (!pts) return '—';
    return (pts / 72).toFixed(3) + ' in';
  }

  let m = $derived(app.metadata);
</script>

{#if m}
  <div class="meta-table-wrap" class:compact>
    <div class="meta-grid" class:compact>
      <div class="meta-cell">
        <div class="mc-label">TrimBox</div>
        <div class="mc-val" class:warn={!m.has_trimbox}>{fmtBox(m.trimbox)}</div>
      </div>
      <div class="meta-cell">
        <div class="mc-label">MediaBox</div>
        <div class="mc-val">{fmtBox(m.mediabox)}</div>
      </div>
      <div class="meta-cell">
        <div class="mc-label">BleedBox</div>
        <div class="mc-val" class:ok={m.has_bleedbox}>{fmtBox(m.bleedbox)}</div>
      </div>
      <div class="meta-cell">
        <div class="mc-label">Bleed</div>
        <div class="mc-val" class:ok={m.bleed_pts > 0}>{fmtBleed(m.bleed_pts)}</div>
      </div>
      {#if !compact}
        <div class="meta-cell">
          <div class="mc-label">Color Space</div>
          <div class="mc-val cs-{m.color_space.toLowerCase()}">{m.color_space}</div>
        </div>
        <div class="meta-cell">
          <div class="mc-label">Pages</div>
          <div class="mc-val">{m.page_count}</div>
        </div>
        <div class="meta-cell">
          <div class="mc-label">File Size</div>
          <div class="mc-val">{formatSize(m.file_size_kb)}</div>
        </div>
        <div class="meta-cell">
          <div class="mc-label">Editing</div>
          <div class="mc-val muted">—</div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .meta-table-wrap {
    padding: 10px 14px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
  }
  .meta-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
  }
  .meta-grid.compact { grid-template-columns: repeat(4, 1fr); }
  .meta-cell {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px 10px;
    min-width: 0;
  }
  .mc-label {
    font-size: 9px;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.1em;
    margin-bottom: 3px;
  }
  .mc-val {
    font-size: 12px;
    color: var(--text);
    font-family: var(--mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mc-val.ok { color: var(--ok); }
  .mc-val.warn { color: var(--warn); }
  .mc-val.muted { color: var(--muted); }
  .mc-val.cs-purecmyk { color: #f97316; }
  .mc-val.cs-purergb { color: #60a5fa; }
  .mc-val.cs-mixed { color: var(--warn); }
</style>
