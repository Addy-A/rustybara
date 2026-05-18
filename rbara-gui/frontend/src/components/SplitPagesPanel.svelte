<script>
  import { useAppState } from '../lib/context.js'
  import Notice from './Notice.svelte'
  import RunButton from './RunButton.svelte'
  const app = useAppState()

  let unit = $state('in')

  const inPresets = [3.675, 5.83, 8.5]
  const mmPresets = [99, 148.5, 210]

  let presets = $derived(unit === 'in' ? inPresets : mmPresets)
  let step = $derived(unit === 'in' ? 0.01 : 0.5)
  let displayValue = $derived(
    unit === 'in'
      ? app.params.splitPanelInches
      : +(app.params.splitPanelInches * 25.4).toFixed(2),
  )
  let panelPts = $derived(app.params.splitPanelInches * 72)
  let otherHint = $derived(
    unit === 'in'
      ? (app.params.splitPanelInches * 25.4).toFixed(1) + ' mm'
      : app.params.splitPanelInches.toFixed(3) + '"',
  )
  let pageCount = $derived(app.metadata?.page_count ?? null)
  let valid = $derived(app.params.splitPanelInches > 0)

  function isPresetActive(p) {
    const inches = unit === 'in' ? p : p / 25.4
    return Math.abs(app.params.splitPanelInches - inches) < 0.005
  }

  function applyPreset(p) {
    app.params.splitPanelInches = unit === 'in' ? p : p / 25.4
  }

  function handleInput(e) {
    const v = parseFloat(e.target.value)
    if (!isNaN(v) && v > 0) {
      app.params.splitPanelInches = unit === 'in' ? v : v / 25.4
    }
  }
</script>

<div class="header">
  <span class="title-icon">⧉</span>
  <div>
    <div class="params-title">Split Pages</div>
    <div class="params-desc">
      Splits each spread page into individual panels at the specified width. All
      panels are saved as pages in a single output file.
    </div>
  </div>
</div>

<div class="param-group">
  <div class="param-label-row">
    <div class="param-label">Panel width</div>
    <div class="unit-toggle">
      <button class:active={unit === 'in'} onclick={() => (unit = 'in')}
        >in</button
      >
      <button class:active={unit === 'mm'} onclick={() => (unit = 'mm')}
        >mm</button
      >
    </div>
  </div>
  <div class="param-row">
    <input
      class="param-input"
      type="number"
      {step}
      min={unit === 'in' ? 0.04 : 1}
      value={displayValue}
      oninput={handleInput}
    />
    <div class="presets">
      {#each presets as p}
        <button
          class="preset-pill"
          class:sel={isPresetActive(p)}
          onclick={() => applyPreset(p)}>{p}</button
        >
      {/each}
    </div>
  </div>
  <div class="param-hint">
    = <span>{panelPts.toFixed(2)}</span> pts · <span>{otherHint}</span>
  </div>
</div>

{#if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else if !valid}
  <Notice ok={false}>Panel width must be greater than 0.</Notice>
{:else}
  <Notice ok
    >Ready — {pageCount} source page{pageCount === 1 ? '' : 's'} →
    <code
      >{unit === 'in'
        ? app.params.splitPanelInches.toFixed(2) + '"'
        : (app.params.splitPanelInches * 25.4).toFixed(1) + 'mm'}</code
    >
    panels → <code>_split.pdf</code></Notice
  >
{/if}

<RunButton label="Split Pages" icon="⧉" disabled={!app.metadata || !valid} />

<style>
  .header {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }
  .title-icon {
    font-size: 20px;
    color: var(--orange);
    flex-shrink: 0;
    padding-top: 1px;
  }
  .params-title {
    font-size: 13px;
    font-weight: 700;
    color: var(--text);
  }
  .params-desc {
    font-size: 11.5px;
    color: var(--muted-hi);
    line-height: 1.55;
    margin-top: 2px;
  }
  .param-group {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .param-label-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .param-label {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--muted-hi);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .unit-toggle {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 4px;
    overflow: hidden;
  }
  .unit-toggle button {
    background: transparent;
    border: none;
    border-left: 1px solid var(--border);
    padding: 2px 7px;
    font-size: 10px;
    font-family: var(--mono);
    color: var(--muted);
    cursor: pointer;
    line-height: 1.6;
  }
  .unit-toggle button:first-child {
    border-left: none;
  }
  .unit-toggle button.active {
    background: var(--orange-dim);
    color: var(--orange-hi);
  }
  .param-row {
    display: flex;
    gap: 8px;
    align-items: center;
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
    width: 110px;
  }
  .param-input:focus {
    border-color: var(--orange);
  }
  .presets {
    display: flex;
    gap: 4px;
  }
  .preset-pill {
    font-size: 10px;
    padding: 4px 9px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--panel);
    color: var(--muted-hi);
    font-family: var(--mono);
    cursor: pointer;
  }
  .preset-pill.sel {
    background: var(--orange-dim);
    color: var(--orange-hi);
    border-color: var(--orange);
  }
  .param-hint {
    font-size: 11px;
    color: var(--muted);
    font-family: var(--mono);
  }
  .param-hint span {
    color: var(--orange);
  }
  code {
    font-family: var(--mono);
    font-size: 10.5px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 0 3px;
  }
</style>
