<script>
  import { useAppState } from '../lib/context.js';
  import { parsePageNums } from '../lib/api.js';
  import Notice from './Notice.svelte';
  import RunButton from './RunButton.svelte';
  const app = useAppState();

  let pageCount = $derived(app.metadata?.page_count ?? null);
  let viewMode = $state('visual');

  // Single source of truth — both modes read/write this
  let parsed = $derived(parsePageNums(app.params.extractPagesInput));
  let validCount = $derived(
    pageCount !== null
      ? parsed.filter(n => n < pageCount).length
      : parsed.length
  );
  let hasInvalid = $derived(pageCount !== null && parsed.some(n => n >= pageCount));

  // Visual mode: toggle a 0-indexed page in the selection
  function togglePage(zeroIdx) {
    const current = new Set(parsed);
    if (current.has(zeroIdx)) current.delete(zeroIdx);
    else current.add(zeroIdx);
    app.params.extractPagesInput = [...current]
      .sort((a, b) => a - b)
      .map(n => n + 1)
      .join(', ') || '';
  }

  // Pages array for visual grid: [1, 2, ... pageCount]
  let pages = $derived(
    pageCount !== null ? Array.from({ length: pageCount }, (_, i) => i + 1) : []
  );
</script>

<div class="header">
  <span class="title-icon">⊟</span>
  <div>
    <div class="params-title">Extract Pages</div>
    <div class="params-desc">Select pages to extract into a new PDF.</div>
  </div>
  <div class="mode-toggle">
    <button
      class="mode-btn"
      class:active={viewMode === 'visual'}
      onclick={() => (viewMode = 'visual')}
    >Visual</button>
    <button
      class="mode-btn"
      class:active={viewMode === 'text'}
      onclick={() => (viewMode = 'text')}
    >Text</button>
  </div>
</div>

{#if viewMode === 'visual'}
  {#if pageCount !== null}
    <div class="grid-wrap">
      <div class="page-grid">
        {#each pages as page (page)}
          {@const idx = page - 1}
          {@const selected = parsed.includes(idx)}
          <button
            class="page-tile"
            class:sel={selected}
            onclick={() => togglePage(idx)}
          >{page}</button>
        {/each}
      </div>
    </div>
    {#if parsed.length > 0}
      <div class="parse-hint">
        → {validCount} page{validCount === 1 ? '' : 's'} selected
        {#if hasInvalid}<span class="warn"> (some numbers exceed page count)</span>{/if}
      </div>
    {/if}
  {:else}
    <div class="no-file-msg">Load a PDF to see pages.</div>
  {/if}
{:else}
  <div class="param-group">
    <div class="param-label">Pages</div>
    <input
      class="param-input"
      type="text"
      placeholder="e.g. 1, 3-5, 7"
      bind:value={app.params.extractPagesInput}
    />
    {#if parsed.length > 0}
      <div class="parse-hint">
        → {validCount} page{validCount === 1 ? '' : 's'} selected
        {#if hasInvalid}<span class="warn"> (some numbers exceed page count)</span>{/if}
      </div>
    {/if}
    <div class="text-hint">1-indexed. Ranges like <code>2-5</code> are supported.</div>
  </div>
{/if}

{#if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else if validCount === 0}
  <Notice ok={false}>No valid pages selected.</Notice>
{:else}
  <Notice ok>Extracting {validCount} of {pageCount} page{pageCount === 1 ? '' : 's'}.</Notice>
{/if}

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

<RunButton label="Extract Pages" icon="⊟" disabled={validCount === 0} />

<style>
  .header { display: flex; align-items: flex-start; gap: 10px; }
  .title-icon { font-size: 20px; color: var(--orange); flex-shrink: 0; padding-top: 1px; }
  .params-title { font-size: 13px; font-weight: 700; color: var(--text); }
  .params-desc { font-size: 11.5px; color: var(--muted-hi); line-height: 1.55; margin-top: 2px; }

  .mode-toggle {
    margin-left: auto;
    display: flex;
    border: 1px solid var(--border);
    border-radius: 5px;
    overflow: hidden;
    flex-shrink: 0;
  }
  .mode-btn {
    padding: 4px 10px;
    font-size: 11px;
    background: var(--panel);
    border: none;
    color: var(--muted-hi);
    cursor: pointer;
  }
  .mode-btn:first-child { border-right: 1px solid var(--border); }
  .mode-btn:hover { color: var(--text); background: var(--surface); }
  .mode-btn.active { background: var(--orange-dim); color: var(--orange-hi); }

  .grid-wrap {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px;
  }
  .page-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .page-tile {
    width: 34px;
    height: 34px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-family: var(--mono);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--muted-hi);
    cursor: pointer;
    transition: 0.1s;
    flex-shrink: 0;
  }
  .page-tile:hover { border-color: var(--orange); color: var(--text); }
  .page-tile.sel {
    background: var(--orange-dim);
    border-color: var(--orange);
    color: var(--orange-hi);
    font-weight: 600;
  }

  .no-file-msg {
    font-size: 11.5px;
    color: var(--muted);
    font-style: italic;
    padding: 8px 0;
  }

  .param-group { display: flex; flex-direction: column; gap: 7px; }
  .param-label {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--muted-hi);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .param-input {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 7px 10px;
    color: var(--text);
    font-family: var(--mono);
    font-size: 12px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }
  .param-input:focus { border-color: var(--orange); }
  .parse-hint { font-size: 11px; color: var(--muted); font-family: var(--mono); }
  .warn { color: #e09f3e; }
  .text-hint {
    font-size: 11px;
    color: var(--muted);
    line-height: 1.5;
  }
  .text-hint code {
    font-family: var(--mono);
    font-size: 10.5px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 0 3px;
  }
  .hint { font-family: var(--mono); font-size: 11px; color: var(--muted); }
</style>
