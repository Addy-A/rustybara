<script>
  import { useAppState } from '../lib/context.js';
  import Notice from './Notice.svelte';
  import RunButton from './RunButton.svelte';
  const app = useAppState();

  function cmykToRgb(c, m, y, k) {
    return `rgb(${~~(255 * (1 - c) * (1 - k))},${~~(255 * (1 - m) * (1 - k))},${~~(255 * (1 - y) * (1 - k))})`;
  }
  let fromBg = $derived(cmykToRgb(...app.params.remapFrom));
  let toBg   = $derived(cmykToRgb(...app.params.remapTo));

  const labels = ['C', 'M', 'Y', 'K'];
</script>

<div class="header">
  <span class="title-icon">⬡</span>
  <div>
    <div class="params-title">Remap Colors</div>
    <div class="params-desc">Replaces every CMYK fill matching the From color (within tolerance) with the To color.</div>
  </div>
</div>

<div class="remap-row">
  <div class="remap-swatch" style="background: {fromBg}"></div>
  <div class="remap-side">
    <div class="remap-side-label">From (CMYK)</div>
    <div class="cmyk-fields">
      {#each labels as L, i (L)}
        <div class="cmyk-field">
          <div class="cmyk-field-label">{L}</div>
          <input
            type="number"
            min="0"
            max="1"
            step="0.01"
            bind:value={app.params.remapFrom[i]}
          />
        </div>
      {/each}
    </div>
  </div>
  <div class="remap-arrow">→</div>
  <div class="remap-side">
    <div class="remap-side-label">To (CMYK)</div>
    <div class="cmyk-fields">
      {#each labels as L, i (L)}
        <div class="cmyk-field">
          <div class="cmyk-field-label">{L}</div>
          <input
            type="number"
            min="0"
            max="1"
            step="0.01"
            bind:value={app.params.remapTo[i]}
          />
        </div>
      {/each}
    </div>
  </div>
  <div class="remap-swatch" style="background: {toBg}"></div>
</div>

<div class="param-group">
  <div class="param-label">Tolerance</div>
  <div class="param-row">
    <input
      class="param-input"
      type="number"
      min="0"
      max="1"
      step="0.01"
      bind:value={app.params.remapTolerance}
    />
    <div class="tolerance-hint">0.0 = exact match · 1.0 = match anything</div>
  </div>
</div>

{#if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else if app.isMixedCs}
  <Notice ok={false}>Mixed color space — only CMYK fills will be remapped.</Notice>
{:else if app.isPureRgb}
  <Notice ok={false}>Pure RGB document — no CMYK fills will be remapped.</Notice>
{:else}
  <Notice ok>CMYK detected — ready to remap.</Notice>
{/if}

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

<RunButton label="Run Remap" icon="⬡" />

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
  .param-row { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .param-input {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 6px 10px;
    color: var(--text);
    font-family: var(--mono);
    font-size: 12px;
    outline: none;
    width: 100px;
  }
  .param-input:focus { border-color: var(--orange); }
  .remap-row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .remap-swatch {
    width: 34px;
    height: 34px;
    border-radius: 5px;
    border: 1px solid var(--border);
    flex-shrink: 0;
    transition: background 0.2s;
  }
  .remap-side {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    min-width: 160px;
  }
  .remap-side-label {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--muted);
  }
  .cmyk-fields { display: flex; gap: 4px; }
  .cmyk-field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
  }
  .cmyk-field-label {
    font-size: 9px;
    color: var(--muted);
    text-align: center;
  }
  .cmyk-field input {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 5px 4px;
    color: var(--text);
    font-family: var(--mono);
    font-size: 11px;
    outline: none;
    text-align: center;
    width: 100%;
  }
  .cmyk-field input:focus { border-color: var(--orange); }
  .remap-arrow {
    color: var(--orange);
    font-size: 20px;
    flex-shrink: 0;
    align-self: flex-end;
    padding-bottom: 4px;
  }
  .tolerance-hint { font-size: 11px; color: var(--muted); }
  .hint { font-family: var(--mono); font-size: 11px; color: var(--muted); }
</style>
