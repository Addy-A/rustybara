<script>
  import { useAppState } from '../lib/context.js';
  import Notice from './Notice.svelte';
  import RunButton from './RunButton.svelte';
  const app = useAppState();

  const formats = ['jpg', 'png', 'webp', 'tiff'];
  const dpiPresets = [72, 150, 300, 600];
</script>

<div class="header">
  <span class="title-icon">⇲</span>
  <div>
    <div class="params-title">Export Images</div>
    <div class="params-desc">Rasterizes every page to an image file.</div>
  </div>
</div>

<div class="param-group">
  <div class="param-label">Format</div>
  <div class="format-grid">
    {#each formats as f}
      <button
        class="format-btn"
        class:sel={app.params.exportFormat === f}
        onclick={() => (app.params.exportFormat = f)}
      >{f.toUpperCase()}</button>
    {/each}
  </div>
</div>

<div class="param-group">
  <div class="param-label">DPI</div>
  <div class="param-row">
    <input
      class="param-input"
      type="number"
      step="1"
      min="36"
      max="1200"
      bind:value={app.params.exportDpi}
    />
    <div class="presets">
      {#each dpiPresets as d}
        <button
          class="preset-pill"
          class:sel={app.params.exportDpi === d}
          onclick={() => (app.params.exportDpi = d)}
        >{d}</button>
      {/each}
    </div>
  </div>
</div>

{#if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else}
  <Notice ok>Ready to export {app.metadata.page_count} page(s) as {app.params.exportFormat.toUpperCase()} @ {app.params.exportDpi} DPI.</Notice>
{/if}

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

<RunButton label="Run Export" icon="⇲" />

<style>
  .header { display: flex; align-items: center; gap: 10px; }
  .title-icon { font-size: 20px; color: var(--orange); }
  .params-title { font-size: 13px; font-weight: 700; color: var(--text); }
  .params-desc { font-size: 11.5px; color: var(--muted-hi); line-height: 1.5; margin-top: 2px; }
  .param-group { display: flex; flex-direction: column; gap: 7px; }
  .param-label {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--muted-hi);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .param-row { display: flex; gap: 8px; align-items: center; }
  .param-input {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 6px 10px;
    color: var(--text);
    font-family: var(--mono);
    font-size: 12px;
    outline: none;
    width: 110px;
  }
  .param-input:focus { border-color: var(--orange); }
  .format-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
    max-width: 320px;
  }
  .format-btn {
    padding: 6px 0;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    font-size: 12px;
    color: var(--muted-hi);
    text-align: center;
    font-family: var(--mono);
  }
  .format-btn.sel {
    background: var(--orange-dim);
    border-color: var(--orange);
    color: var(--orange-hi);
  }
  .format-btn:hover { border-color: var(--border-hi); color: var(--text); }
  .presets { display: flex; gap: 4px; }
  .preset-pill {
    font-size: 10px;
    padding: 4px 9px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--panel);
    color: var(--muted-hi);
    font-family: var(--mono);
  }
  .preset-pill.sel {
    background: var(--orange-dim);
    color: var(--orange-hi);
    border-color: var(--orange);
  }
  .hint { font-family: var(--mono); font-size: 11px; color: var(--muted); }
</style>
