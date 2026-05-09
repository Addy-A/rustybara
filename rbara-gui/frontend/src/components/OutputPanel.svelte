<script>
  import { useAppState } from '../lib/context.js';
  import { open } from '@tauri-apps/plugin-dialog';
  const app = useAppState();

  async function pickDir() {
    const sel = await open({ directory: true, multiple: false });
    if (typeof sel === 'string') app.outputDir = sel;
  }
</script>

<div class="header">
  <span class="title-icon">⊘</span>
  <div>
    <div class="params-title">Output Path</div>
    <div class="params-desc">Where processed files are written. Leave unset to use each file's source folder.</div>
  </div>
</div>

<div class="param-group">
  <div class="param-label">Mode</div>
  <div class="param-seg">
    <button
      class="seg-btn"
      class:sel={!app.overwrite && !app.outputDir}
      onclick={() => { app.overwrite = false; app.outputDir = null; }}
    >Same folder (_processed)</button>
    <button
      class="seg-btn"
      class:sel={!app.overwrite && !!app.outputDir}
      onclick={() => { app.overwrite = false; pickDir(); }}
    >Custom folder…</button>
    <button
      class="seg-btn"
      class:sel={app.overwrite}
      onclick={() => { app.overwrite = true; }}
    >Overwrite source</button>
  </div>
</div>

<div class="param-group">
  <div class="param-label">Current</div>
  <div class="path-display">
    {app.overwrite ? '⚠ Overwriting source files in place' : (app.outputDir ?? '— same folder as each input —')}
  </div>
</div>

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

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
  .param-seg {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 5px;
    overflow: hidden;
    background: var(--panel);
    align-self: flex-start;
    flex-wrap: wrap;
  }
  .seg-btn {
    padding: 5px 12px;
    font-size: 12px;
    color: var(--muted-hi);
    border: none;
    border-right: 1px solid var(--border);
    background: transparent;
    font-family: var(--sans);
    white-space: nowrap;
  }
  .seg-btn:last-child { border-right: none; }
  .seg-btn.sel { background: var(--orange-dim); color: var(--orange-hi); }
  .path-display {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--muted-hi);
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 8px 10px;
    word-break: break-all;
  }
  .hint { font-family: var(--mono); font-size: 11px; color: var(--muted); }
</style>
