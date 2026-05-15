<script>
  import { useAppState } from '../lib/context.js';
  import { parsePageNums } from '../lib/api.js';
  import Notice from './Notice.svelte';
  import RunButton from './RunButton.svelte';
  const app = useAppState();

  let pageCount = $derived(app.metadata?.page_count ?? null);
  let parsed = $derived(parsePageNums(app.params.extractPagesInput));
  let validCount = $derived(
    pageCount !== null
      ? parsed.filter(n => n < pageCount).length
      : parsed.length
  );
  let hasInvalid = $derived(pageCount !== null && parsed.some(n => n >= pageCount));
</script>

<div class="header">
  <span class="title-icon">⊟</span>
  <div>
    <div class="params-title">Extract Pages</div>
    <div class="params-desc">Extracts specific pages into a new PDF. Enter 1-indexed page numbers separated by commas. Ranges like <code>2-5</code> are supported.</div>
  </div>
</div>

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
</div>

{#if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else if validCount === 0}
  <Notice ok={false}>No valid pages selected — check your input.</Notice>
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
  .params-desc code {
    font-family: var(--mono);
    font-size: 10.5px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 0 3px;
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
  .hint { font-family: var(--mono); font-size: 11px; color: var(--muted); }
</style>
