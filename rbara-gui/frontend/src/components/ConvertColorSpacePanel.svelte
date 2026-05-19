<script>
  import { useAppState } from '../lib/context.js'
  import { loadIccProfile, BUILTIN_PROFILES } from '../lib/api.js'
  import Notice from './Notice.svelte'
  import RunButton from './RunButton.svelte'
  const app = useAppState()

  let cmykProfiles = $derived(BUILTIN_PROFILES.filter((p) => p.color_space === 'CMYK'))
  let rgbProfiles  = $derived(BUILTIN_PROFILES.filter((p) => p.color_space === 'RGB'))

  const intents = [
    {
      value: 'RelativeColorimetric',
      label: 'Relative Colorimetric',
      desc: 'Best for print. In-gamut colors stay exact; out-of-gamut colors clip to the nearest match. White point adapts to the destination.',
    },
    {
      value: 'Perceptual',
      label: 'Perceptual',
      desc: 'Scales the whole gamut to fit, preserving visual relationships. Colors may shift slightly but nothing clips. Good for photos.',
    },
    {
      value: 'Saturation',
      label: 'Saturation',
      desc: 'Keeps colors vibrant at the expense of accuracy. Suited for charts, logos, and graphics where punchy color matters most.',
    },
    {
      value: 'AbsoluteColorimetric',
      label: 'Absolute Colorimetric',
      desc: 'Like Relative but does not adjust the white point. Used for soft-proofing to simulate one substrate on another.',
    },
  ]

  let sameProfile = $derived(app.params.fromProfile === app.params.toProfile)
  let importing = $state(false)
  let importError = $state('')

  async function importProfile() {
    importing = true
    importError = ''
    try {
      const dtos = await loadIccProfile()
      for (const dto of dtos) {
        app.addCustomProfile(dto)
      }
      if (dtos.length > 0) {
        app.logAction({
          ok: true,
          message: `Imported ${dtos.length} ICC profile(s): ${dtos.map((d) => d.description).join(', ')}`,
          output_paths: [],
          timestamp: new Date().toLocaleTimeString(),
          action: 'ImportICC',
        })
      }
    } catch (e) {
      const msg = typeof e === 'string' ? e : String(e)
      importError = msg
      app.logAction({
        ok: false,
        message: msg,
        output_paths: [],
        timestamp: new Date().toLocaleTimeString(),
        action: 'ImportICC',
      })
    } finally {
      importing = false
    }
  }

  let customCmyk = $derived(
    app.customProfiles.filter((p) => p.color_space === 'CMYK'),
  )
  let customRgb = $derived(
    app.customProfiles.filter((p) => p.color_space === 'RGB'),
  )
  let customOther = $derived(
    app.customProfiles.filter(
      (p) => p.color_space !== 'CMYK' && p.color_space !== 'RGB',
    ),
  )
</script>

<div class="header">
  <span class="title-icon">◈</span>
  <div class="header-text">
    <div class="params-title">Convert Color Space</div>
    <div class="params-desc">
      Applies an ICC transform to every CMYK/RGB paint operator in the
      document's content streams.
    </div>
  </div>
  <button class="import-btn" disabled={importing} onclick={importProfile}>
    {importing ? '…' : '+ Import ICC'}
  </button>
</div>

<div class="param-group">
  <div class="param-label">Source Profile</div>
  <select class="param-select" bind:value={app.params.fromProfile}>
    <optgroup label="CMYK">
      {#each cmykProfiles as p (p.value)}
        <option value={p.value}>{p.label}</option>
      {/each}
      {#each customCmyk as p (p.name)}
        <option value={p.name}>{p.description} ★</option>
      {/each}
    </optgroup>
    <optgroup label="RGB">
      {#each rgbProfiles as p (p.value)}
        <option value={p.value}>{p.label}</option>
      {/each}
      {#each customRgb as p (p.name)}
        <option value={p.name}>{p.description} ★</option>
      {/each}
    </optgroup>
    {#if customOther.length > 0}
      <optgroup label="Other">
        {#each customOther as p (p.name)}
          <option value={p.name}>{p.description} ★</option>
        {/each}
      </optgroup>
    {/if}
  </select>
</div>

<div class="arrow-row">
  <span class="arrow">↓</span>
</div>

<div class="param-group">
  <div class="param-label">Destination Profile</div>
  <select class="param-select" bind:value={app.params.toProfile}>
    <optgroup label="CMYK">
      {#each cmykProfiles as p (p.value)}
        <option value={p.value}>{p.label}</option>
      {/each}
      {#each customCmyk as p (p.name)}
        <option value={p.name}>{p.description} ★</option>
      {/each}
    </optgroup>
    <optgroup label="RGB">
      {#each rgbProfiles as p (p.value)}
        <option value={p.value}>{p.label}</option>
      {/each}
      {#each customRgb as p (p.name)}
        <option value={p.name}>{p.description} ★</option>
      {/each}
    </optgroup>
    {#if customOther.length > 0}
      <optgroup label="Other">
        {#each customOther as p (p.name)}
          <option value={p.name}>{p.description} ★</option>
        {/each}
      </optgroup>
    {/if}
  </select>
</div>

<div class="param-group">
  <div class="param-label">Rendering Intent</div>
  <div class="intent-list">
    {#each intents as i (i.value)}
      <button
        class="intent-card"
        class:sel={app.params.convertIntent === i.value}
        onclick={() => (app.params.convertIntent = i.value)}
      >
        <div class="intent-dot"></div>
        <div class="intent-text">
          <div class="intent-name">{i.label}</div>
          <div class="intent-desc">{i.desc}</div>
        </div>
      </button>
    {/each}
  </div>
</div>

{#if importError}
  <Notice ok={false}>{importError}</Notice>
{:else if sameProfile}
  <Notice ok={false}
    >Source and destination profiles are the same — no conversion will occur.</Notice
  >
{:else if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else}
  <Notice ok>Ready to convert.</Notice>
{/if}

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

<RunButton label="Run Conversion" icon="◈" />

<style>
  .header {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }
  .header-text {
    flex: 1;
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
    line-height: 1.5;
    margin-top: 2px;
  }
  .param-group {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .param-label {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--muted-hi);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .import-btn {
    font-size: 10px;
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--panel);
    color: var(--muted-hi);
    cursor: pointer;
    font-family: var(--mono);
  }
  .import-btn:hover:not(:disabled) {
    border-color: var(--orange);
    color: var(--orange-hi);
  }
  .import-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .param-select {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 7px 10px;
    color: var(--text);
    font-size: 12px;
    outline: none;
    width: 100%;
  }
  .param-select:focus {
    border-color: var(--orange);
  }
  .arrow-row {
    display: flex;
    justify-content: center;
    padding: 2px 0;
  }
  .arrow {
    font-size: 20px;
    color: var(--orange);
  }
  .hint {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--muted);
  }

  .intent-list {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .intent-card {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 8px 10px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    text-align: left;
    cursor: pointer;
    transition: 0.1s;
    width: 100%;
  }
  .intent-card:hover {
    border-color: var(--muted-hi);
  }
  .intent-card.sel {
    border-color: var(--orange);
    background: var(--orange-dim);
  }
  .intent-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--border);
    background: transparent;
    flex-shrink: 0;
    margin-top: 3px;
    transition: 0.1s;
  }
  .intent-card.sel .intent-dot {
    border-color: var(--orange);
    background: var(--orange);
  }
  .intent-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .intent-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
  }
  .intent-card.sel .intent-name {
    color: var(--orange-hi);
  }
  .intent-desc {
    font-size: 11px;
    color: var(--muted-hi);
    line-height: 1.5;
  }
</style>
