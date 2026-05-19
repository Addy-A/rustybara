<script>
  import { useAppState } from '../lib/context.js'
  import { colorSpaceLabel, colorSpaceTagClass } from '../lib/api.js'

  const app = useAppState()
  let f = $derived(app.activeFileObj)
  let m = $derived(app.metadata)
</script>

<div class="preview-strip">
  {#if f}
    {#each Array(Math.min(3, m?.page_count ?? 1)) as _, i (i)}
      <div class="page-stack">
        <div
          class="preview-page"
          class:active={i === 0}
          style="opacity:{1 - i * 0.22}"
        >
          <div class="preview-page-inner">
            {#each Array(8) as _, j (j)}
              <div class="pl"></div>
            {/each}
          </div>
        </div>
        <div class="page-badge">
          p.{i + 1}{i === 0 && m?.page_count ? ` / ${m.page_count}` : ''}
        </div>
      </div>
    {/each}
    <div class="preview-meta">
      <div class="preview-filename">{f.name}</div>
      <div class="preview-tags">
        <span class="tag {colorSpaceTagClass(f.colorSpace)}"
          >{colorSpaceLabel(f.colorSpace)}</span
        >
        {#if m?.has_spots}
          {#each (m?.spot_colors ?? []) as name}
            <span class="tag spot">✦ {name}</span>
          {/each}
        {/if}
        {#if m?.has_trimbox}
          <span class="tag ok">TrimBox ✓</span>
        {:else}
          <span class="tag warn">No TrimBox</span>
        {/if}
        {#if m?.has_bleedbox}
          <span class="tag ok">Bleed ✓</span>
        {/if}
      </div>
      <div class="filesize">{f.sizeKb} KB · {m?.page_count ?? '?'} page(s)</div>
    </div>
  {:else}
    <div class="empty">
      <div class="empty-line">No file loaded</div>
    </div>
  {/if}
</div>

<style>
  .preview-strip {
    height: 134px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    flex-shrink: 0;
    overflow: hidden;
  }
  .page-stack {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    flex-shrink: 0;
  }
  .preview-page {
    height: 104px;
    aspect-ratio: 8.5 / 11;
    background: #fff;
    border-radius: 2px;
    box-shadow: 0 2px 10px #00000066;
    position: relative;
    flex-shrink: 0;
  }
  .preview-page.active {
    outline: 2px solid var(--orange);
  }
  .preview-page-inner {
    position: absolute;
    inset: 0;
    background: linear-gradient(135deg, #f0f0f0, #e0e0e0);
    border-radius: 2px;
    display: flex;
    flex-direction: column;
    padding: 5px;
    gap: 2px;
    overflow: hidden;
  }
  .pl {
    height: 2px;
    background: #bbb;
    border-radius: 1px;
  }
  .page-badge {
    font-size: 9px;
    color: var(--muted);
    font-family: var(--mono);
    text-align: center;
    white-space: nowrap;
  }
  .preview-meta {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding-left: 10px;
    overflow: hidden;
    min-width: 0;
  }
  .preview-filename {
    font-size: 14px;
    font-weight: 700;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .preview-tags {
    display: flex;
    gap: 5px;
    flex-wrap: wrap;
  }
  .filesize {
    font-size: 11px;
    color: var(--muted);
    font-family: var(--mono);
  }
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
  }
  .empty-line {
    font-size: 13px;
    color: var(--muted-hi);
  }
  .empty-quip {
    font-size: 11px;
    color: var(--orange);
    font-family: var(--mono);
    font-style: italic;
  }
</style>
