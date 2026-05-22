<script>
  import { useAppState } from '../lib/context.js'
  import { loadIccProfile } from '../lib/api.js'
  import Notice from './Notice.svelte'
  import RunButton from './RunButton.svelte'
  const app = useAppState()

  let importing = $state(false)
  let importError = $state('')

  let customCmyk = $derived(
    app.customProfiles.filter((p) => p.color_space === 'CMYK'),
  )

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
      importError = typeof e === 'string' ? e : String(e)
    } finally {
      importing = false
    }
  }
</script>

<div class="header">
  <span class="title-icon">✦</span>
  <div class="header-text">
    <div class="params-title">Flatten Spot Colors</div>
    <div class="params-desc">
      Replaces <code>Separation</code> spot ink operators with device CMYK equivalents,
      evaluated from each spot color's embedded tint function. When a destination
      ICC profile is set, Lab alternate-space values are converted through that profile;
      otherwise the bundled US Web Coated SWOP v2 profile is used.
    </div>
  </div>
  <button class="import-btn" disabled={importing} onclick={importProfile}>
    {importing ? '…' : '+ Import ICC'}
  </button>
</div>

<div class="info-box">
  <div class="info-row">
    <span class="info-label">Input</span>
    <span class="info-val"
      >Separation color space <code>cs</code> / <code>scn</code> operators</span
    >
  </div>
  <div class="info-row">
    <span class="info-label">Output</span>
    <span class="info-val"
      >Device CMYK <code>k</code> / <code>K</code> operators</span
    >
  </div>
  <div class="info-row">
    <span class="info-label">Note</span>
    <span class="info-val"
      >DeviceN multi-channel inks are detected but not flattened.</span
    >
  </div>
</div>

<div class="param-group">
  <div class="param-label">Destination Profile</div>
  <select
    class="param-select"
    value={app.params.spotIccProfile ?? ''}
    onchange={(e) => {
      app.params.spotIccProfile = e.target.value || null
    }}
  >
    <option value="">default (US Web Coated SWOP)</option>
    {#if customCmyk.length > 0}
      <optgroup label="Custom CMYK">
        {#each customCmyk as p (p.name)}
          <option value={p.name}>{p.description} ★</option>
        {/each}
      </optgroup>
    {/if}
  </select>
</div>

{#if importError}
  <Notice ok={false}>{importError}</Notice>
{:else if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else if app.metadata.color_space === 'PureRGB'}
  <Notice ok={false}>Pure RGB document — no spot colors expected.</Notice>
{:else}
  <Notice ok>Ready to flatten spot colors.</Notice>
{/if}

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

<RunButton label="Flatten Spots" icon="✦" />

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
  .info-row {
    display: flex;
    gap: 10px;
    font-size: 11.5px;
  }
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
  .info-val {
    color: var(--muted-hi);
    line-height: 1.45;
  }
  .info-val code {
    font-family: var(--mono);
    font-size: 10.5px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 0 3px;
  }
  .hint {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--muted);
  }
  .header {
    align-items: flex-start;
  }
  .header-text {
    flex: 1;
  }
  .import-btn {
    flex-shrink: 0;
    font-size: 11px;
    font-weight: 600;
    padding: 4px 10px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--panel);
    color: var(--muted-hi);
    cursor: pointer;
    white-space: nowrap;
  }
  .import-btn:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }
  .import-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .param-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .param-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--muted);
  }
  .param-select {
    width: 100%;
    font-size: 12px;
    font-family: var(--mono);
    padding: 5px 8px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--panel);
    color: var(--text);
    cursor: pointer;
  }
  .param-select:focus {
    outline: none;
    border-color: var(--accent);
  }
</style>
