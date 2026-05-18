<script>
  import { useAppState } from '../lib/context.js'
  import Notice from './Notice.svelte'
  import RunButton from './RunButton.svelte'
  const app = useAppState()

  let unit = $state('in')

  const inPresets = [8.5, 11, 17]
  const mmPresets = [210, 297, 420]

  let presets = $derived(unit === 'in' ? inPresets : mmPresets)
  let step = $derived(unit === 'in' ? 0.01 : 0.5)
  let displayValue = $derived(
    unit === 'in'
      ? app.params.stitchSpreadInches
      : +(app.params.stitchSpreadInches * 25.4).toFixed(2),
  )
  let spreadPts = $derived(app.params.stitchSpreadInches * 72)
  let otherHint = $derived(
    unit === 'in'
      ? (app.params.stitchSpreadInches * 25.4).toFixed(1) + ' mm'
      : app.params.stitchSpreadInches.toFixed(3) + '"',
  )
  let pageCount = $derived(app.metadata?.page_count ?? null)
  let valid = $derived(app.params.stitchSpreadInches > 0)

  // spread count estimate
  let pageBox = $derived(
    app.metadata?.trimbox ?? app.metadata?.mediabox ?? null,
  )
  let pw = $derived(pageBox ? pageBox[2] - pageBox[0] : 0)
  let ph = $derived(pageBox ? pageBox[3] - pageBox[1] : 0)
  let panelsPerSpread = $derived(
    pw > 0 && spreadPts > 0 ? Math.max(1, Math.round(spreadPts / pw)) : null,
  )
  let spreadCount = $derived(
    pageCount != null && panelsPerSpread != null
      ? Math.ceil(pageCount / panelsPerSpread)
      : null,
  )

  // diagram: show panels side by side up to panelsPerSpread
  let diagramPanels = $derived(
    panelsPerSpread != null ? Math.min(panelsPerSpread, 8) : 0,
  )

  function isPresetActive(p) {
    const inches = unit === 'in' ? p : p / 25.4
    return Math.abs(app.params.stitchSpreadInches - inches) < 0.005
  }

  function applyPreset(p) {
    app.params.stitchSpreadInches = unit === 'in' ? p : p / 25.4
  }

  function handleInput(e) {
    const v = parseFloat(e.target.value)
    if (!isNaN(v) && v > 0) {
      app.params.stitchSpreadInches = unit === 'in' ? v : v / 25.4
    }
  }
</script>

<div class="header">
  <span class="title-icon">⧈</span>
  <div>
    <div class="params-title">Stitch Pages</div>
    <div class="params-desc">
      Combines consecutive panels side-by-side into spread pages at the
      specified width. The number of panels per spread is inferred from the
      source page width. Output is always named
      <code>_stitch.pdf</code> — the source file is never overwritten. The
      overwrite toggle replaces any existing <code>_stitch</code> file.
    </div>
  </div>
</div>

<div class="param-group">
  <div class="param-label-row">
    <div class="param-label">Spread width</div>
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
    <div class="input-wrap">
      <input
        class="param-input"
        class:has-suffix={unit === 'in'}
        type="number"
        {step}
        min={unit === 'in' ? 0.04 : 1}
        value={displayValue}
        oninput={handleInput}
      />
      {#if unit === 'in'}<span class="input-suffix">"</span>{/if}
    </div>
    <div class="presets">
      {#each presets as p}
        <button
          class="preset-pill"
          class:sel={isPresetActive(p)}
          onclick={() => applyPreset(p)}>{p}{unit === 'in' ? '"' : ''}</button
        >
      {/each}
    </div>
  </div>
  <div class="param-hint">
    = <span>{spreadPts.toFixed(2)}</span> pts · <span>{otherHint}</span>
    {#if panelsPerSpread != null}
      · <span>{panelsPerSpread}</span> panel{panelsPerSpread === 1
        ? ''
        : 's'}/spread
    {/if}
  </div>
</div>

{#if app.metadata && pw > 0 && diagramPanels > 0}
  <div class="diagram-wrap">
    <svg
      viewBox="0 0 {pw * diagramPanels} {ph}"
      width="100%"
      style="max-height: 160px; display: block;"
      preserveAspectRatio="xMidYMid meet"
    >
      {#each Array.from({ length: diagramPanels }) as _, i}
        <!-- panel background -->
        <rect
          x={pw * i}
          y="0"
          width={pw}
          height={ph}
          fill="var(--panel)"
          rx={ph * 0.012}
        />
        <!-- panel border -->
        <rect
          x={pw * i}
          y="0"
          width={pw}
          height={ph}
          fill="none"
          stroke="var(--orange)"
          stroke-width={Math.max(pw, ph) * 0.004}
          rx={ph * 0.012}
        />
        <!-- panel number label -->
        <text
          x={pw * i + pw / 2}
          y={ph / 2}
          text-anchor="middle"
          dominant-baseline="middle"
          font-size={ph * 0.12}
          fill="var(--muted)">{i + 1}</text
        >
      {/each}

      <!-- spread boundary line covering full spread -->
      <rect
        x="0"
        y="0"
        width={pw * diagramPanels}
        height={ph}
        fill="none"
        stroke="var(--orange-hi)"
        stroke-width={Math.max(pw, ph) * 0.006}
        stroke-dasharray={Math.max(pw, ph) * 0.02}
        rx={ph * 0.012}
      />
    </svg>
  </div>
{/if}

{#if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else if !valid}
  <Notice ok={false}>Spread width must be greater than 0.</Notice>
{:else}
  <Notice ok
    >Ready — {pageCount} panel{pageCount === 1 ? '' : 's'} →
    {#if spreadCount != null}
      <span>{spreadCount} spread{spreadCount === 1 ? '' : 's'}</span> ·
    {/if}
    <code
      >{unit === 'in'
        ? app.params.stitchSpreadInches.toFixed(2) + '"'
        : (app.params.stitchSpreadInches * 25.4).toFixed(1) + 'mm'}</code
    >
    wide → <code>_stitch.pdf</code></Notice
  >
{/if}

<RunButton label="Stitch Pages" icon="⧈" disabled={!app.metadata || !valid} />

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
  .param-input.has-suffix {
    padding-right: 22px;
  }
  .param-input:focus {
    border-color: var(--orange);
  }
  .input-wrap {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .input-suffix {
    position: absolute;
    right: 8px;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--muted);
    pointer-events: none;
    user-select: none;
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
  .diagram-wrap {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px;
    display: flex;
    justify-content: center;
  }
</style>
