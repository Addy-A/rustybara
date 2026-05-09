<script>
  import { useAppState } from '../lib/context.js';
  import Notice from './Notice.svelte';
  import RunButton from './RunButton.svelte';
  const app = useAppState();

  const presets = [0.0625, 0.125, 0.25];
</script>

<div class="header">
  <span class="title-icon">⊡</span>
  <div>
    <div class="params-title">Resize to Bleed</div>
    <div class="params-desc">Expands MediaBox outward by the bleed amount around the TrimBox.</div>
  </div>
</div>

<div class="param-group">
  <div class="param-label">Bleed (inches)</div>
  <div class="param-row">
    <input
      class="param-input"
      type="number"
      step="0.001"
      min="0"
      bind:value={app.params.bleedInches}
    />
    <div class="presets">
      {#each presets as p}
        <button
          class="preset-pill"
          class:sel={Math.abs(app.params.bleedInches - p) < 1e-6}
          onclick={() => (app.params.bleedInches = p)}
        >{p}″</button>
      {/each}
    </div>
  </div>
  <div class="param-hint">= <span>{(app.params.bleedInches * 72).toFixed(2)}</span> pts</div>
</div>

{#if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else if !app.canResize}
  <Notice ok={false}>No TrimBox detected — cannot compute bleed expansion.</Notice>
{:else}
  <Notice ok>TrimBox detected — ready to expand by {app.params.bleedInches}″.</Notice>
{/if}

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

<RunButton label="Run Resize" icon="⊡" disabled={!app.canResize} />

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
  .param-hint { font-size: 11px; color: var(--muted); font-family: var(--mono); }
  .param-hint span { color: var(--orange); }
  .hint { font-family: var(--mono); font-size: 11px; color: var(--muted); }
</style>
