<script>
  import { useAppState } from '../lib/context.js';
  import Notice from './Notice.svelte';
  import RunButton from './RunButton.svelte';
  const app = useAppState();

  // Common page sizes in inches [w, h]. Setting a preset fills both fields.
  const presets = [
    { label: 'Letter', w: 8.5, h: 11 },
    { label: 'A4', w: 8.27, h: 11.69 },
    { label: 'Legal', w: 8.5, h: 14 },
  ];

  function applyPreset(p) {
    app.params.setMediaBoxWidthInches = p.w;
    app.params.setMediaBoxHeightInches = p.h;
  }

  function isPreset(p) {
    return (
      Math.abs(app.params.setMediaBoxWidthInches - p.w) < 1e-6 &&
      Math.abs(app.params.setMediaBoxHeightInches - p.h) < 1e-6
    );
  }

  let wIn = $derived(app.params.setMediaBoxWidthInches);
  let hIn = $derived(app.params.setMediaBoxHeightInches);
  let invalid = $derived(!app.metadata || wIn <= 0 || hIn <= 0);

  // Current page size from load_metadata's mediabox = [x0, y0, x1, y1] in points.
  let curWIn = $derived(
    app.metadata ? (app.metadata.mediabox[2] - app.metadata.mediabox[0]) / 72 : 0,
  );
  let curHIn = $derived(
    app.metadata ? (app.metadata.mediabox[3] - app.metadata.mediabox[1]) / 72 : 0,
  );
  // Whether the new box trims the page (smaller in either dimension) — content
  // outside the new MediaBox is cropped by the viewer/RIP.
  let crops = $derived(
    !!app.metadata && (wIn < curWIn - 1e-6 || hIn < curHIn - 1e-6),
  );
</script>

<div class="header">
  <span class="title-icon">▭</span>
  <div>
    <div class="params-title">Set Media Box</div>
    <div class="params-desc">Sets every page's MediaBox to an exact width × height, centered on the current media. Content outside the new box is cropped.</div>
  </div>
</div>

<div class="dims">
  <div class="param-group">
    <div class="param-label">Width (inches)</div>
    <input
      class="param-input"
      type="number"
      step="0.001"
      min="0"
      bind:value={app.params.setMediaBoxWidthInches}
    />
    <div class="param-hint">= <span>{(wIn * 72).toFixed(2)}</span> pts</div>
  </div>
  <div class="param-group">
    <div class="param-label">Height (inches)</div>
    <input
      class="param-input"
      type="number"
      step="0.001"
      min="0"
      bind:value={app.params.setMediaBoxHeightInches}
    />
    <div class="param-hint">= <span>{(hIn * 72).toFixed(2)}</span> pts</div>
  </div>
</div>

<div class="presets">
  {#each presets as p}
    <button
      class="preset-pill"
      class:sel={isPreset(p)}
      onclick={() => applyPreset(p)}
    >{p.label}</button>
  {/each}
</div>

{#if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else if wIn <= 0 || hIn <= 0}
  <Notice ok={false}>Width and height must both be greater than 0″.</Notice>
{:else}
  <Notice ok>
    Current page is {curWIn.toFixed(2)}×{curHIn.toFixed(2)}″ — setting to {wIn}×{hIn}″{crops ? ' (edges will be cropped)' : ''}.
  </Notice>
{/if}

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

<RunButton label="Set Media Box" icon="▭" disabled={invalid} />

<style>
  .header { display: flex; align-items: center; gap: 10px; }
  .title-icon { font-size: 20px; color: var(--orange); }
  .params-title { font-size: 13px; font-weight: 700; color: var(--text); }
  .params-desc { font-size: 11.5px; color: var(--muted-hi); line-height: 1.5; margin-top: 2px; }
  .dims { display: flex; gap: 12px; }
  .param-group { display: flex; flex-direction: column; gap: 7px; flex: 1; }
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
    padding: 6px 10px;
    color: var(--text);
    font-family: var(--mono);
    font-size: 12px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
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
