<script>
  import { useAppState } from '../lib/context.js';
  import Notice from './Notice.svelte';
  import RunButton from './RunButton.svelte';
  const app = useAppState();

  let pageCount = $derived(app.metadata?.page_count ?? null);
</script>

<div class="header">
  <span class="title-icon">⧉</span>
  <div>
    <div class="params-title">Split Pages</div>
    <div class="params-desc">Saves every page as its own PDF file. Output files are named <code>filename_page_1.pdf</code>, <code>_page_2.pdf</code>, etc.</div>
  </div>
</div>

{#if pageCount !== null}
  <div class="info-box">
    <div class="info-row">
      <span class="info-label">Pages</span>
      <span class="info-val">{pageCount} page{pageCount === 1 ? '' : 's'} → {pageCount} file{pageCount === 1 ? '' : 's'}</span>
    </div>
    <div class="info-row">
      <span class="info-label">Output</span>
      <span class="info-val">Written to the output directory (or same folder as source)</span>
    </div>
  </div>
{/if}

{#if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else if pageCount === 1}
  <Notice ok={false}>Document has only 1 page — nothing to split.</Notice>
{:else}
  <Notice ok>Ready to split into {pageCount} file{pageCount === 1 ? '' : 's'}.</Notice>
{/if}

<RunButton label="Split Pages" icon="⧉" disabled={pageCount !== null && pageCount <= 1} />

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
  .info-box {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .info-row { display: flex; gap: 10px; font-size: 11.5px; }
  .info-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--muted);
    width: 46px;
    flex-shrink: 0;
    padding-top: 1px;
  }
  .info-val { color: var(--muted-hi); line-height: 1.45; }
</style>
