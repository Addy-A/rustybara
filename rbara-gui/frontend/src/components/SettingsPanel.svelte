<script>
  import { useAppState } from '../lib/context.js'
  import { RESERVED_KEYS } from '../lib/shortcuts.js'
  import { themes, sansFonts, monoFonts, applyTheme } from '../lib/themes.js'
  const app = useAppState()

  function cloneSettings(s) {
    return s ? JSON.parse(JSON.stringify(s)) : {}
  }

  let draft = $state(cloneSettings(app.settings))
  let saved = $state(false)
  let saveError = $state(null)
  let newQuip = $state('')

  // Sync draft when settings first loads (null → object) or is reset externally.
  let prevSettings = app.settings
  $effect(() => {
    if (app.settings && app.settings !== prevSettings) {
      prevSettings = app.settings
      if (!saved) draft = cloneSettings(app.settings)
    }
  })

  // Live-preview theme changes as the user browses presets.
  $effect(() => {
    if (draft.theme_preset) applyTheme(draft.theme_preset, draft.font_sans, draft.font_mono)
  })

  async function save() {
    saveError = null
    try {
      await app.saveSettings(draft)
      saved = true
      setTimeout(() => (saved = false), 2000)
    } catch (e) {
      saveError = String(e)
    }
  }

  function reset() {
    if (!app.settings) return
    draft = cloneSettings(app.settings)
    saveError = null
  }

  // ── Theme preset grid ────────────────────────────────────────────────────────

  // Group themes into dark/light pairs for the grid.
  const darkThemes  = themes.filter(t => t.dark)
  const lightThemes = themes.filter(t => !t.dark)

  // ── Shortcuts ────────────────────────────────────────────────────────────────

  const ACTION_DEFS = [
    { id: 'trim',         label: 'Trim Marks',      default: 't' },
    { id: 'resize',       label: 'Resize to Bleed', default: 'r' },
    { id: 'export',       label: 'Export Images',   default: 'x' },
    { id: 'remap',        label: 'Remap Colors',    default: 'm' },
    { id: 'colorspace',   label: 'Color Space',     default: 'c' },
    { id: 'spots',        label: 'Flatten Spots',   default: 's' },
    { id: 'addtrimbox',   label: 'Add Trim Box',    default: 'b' },
    { id: 'setmediabox',  label: 'Set Media Box',   default: 'M' },
    { id: 'outlinetext',  label: 'Outline Text',    default: 'T' },
    { id: 'splitpages',   label: 'Split Pages',     default: 'p' },
    { id: 'stitchpages',  label: 'Stitch Pages',    default: 'g' },
    { id: 'extractpages', label: 'Extract Pages',   default: 'e' },
  ]

  function effectiveKey(id) {
    return draft.shortcuts?.[id] ?? ACTION_DEFS.find(a => a.id === id)?.default ?? ''
  }

  function setKey(id, value) {
    const def = ACTION_DEFS.find(a => a.id === id)?.default
    const next = { ...(draft.shortcuts ?? {}) }
    if (!value || value === def) {
      delete next[id]
    } else {
      next[id] = value
    }
    draft.shortcuts = next
  }

  function conflictFor(id, key) {
    if (!key) return null
    if (RESERVED_KEYS.includes(key)) return 'reserved key'
    for (const def of ACTION_DEFS) {
      if (def.id === id) continue
      if (effectiveKey(def.id) === key) return def.label
    }
    return null
  }

  // ── Quips ────────────────────────────────────────────────────────────────────

  function addQuip() {
    const q = newQuip.trim()
    if (!q) return
    draft.custom_quips = [...(draft.custom_quips ?? []), q]
    newQuip = ''
  }

  function removeQuip(i) {
    draft.custom_quips = draft.custom_quips.filter((_, idx) => idx !== i)
    if (draft.custom_quips.length === 0) draft.custom_quips = null
  }

  function resetQuips() {
    draft.custom_quips = null
  }

  // ── Misc ─────────────────────────────────────────────────────────────────────

  const dpiPresets    = [72, 150, 300, 600]
  const formatOptions = ['jpg', 'png', 'webp', 'tiff']
  const intentOptions = ['RelativeColorimetric', 'Perceptual', 'Saturation', 'AbsoluteColorimetric']
  const layoutOptions = [
    { value: null,       label: 'Auto (window width)' },
    { value: 'wide',     label: 'Wide' },
    { value: 'square',   label: 'Square' },
    { value: 'vertical', label: 'Vertical' },
  ]

  // Derive the current resolved layout for the live label.
  let resolvedLayout = $derived.by(() => {
    if (draft.layout_override) return draft.layout_override
    const w = window.innerWidth
    const h = window.innerHeight
    const bp = draft.wide_breakpoint_px ?? 900
    return w > bp ? 'wide' : h > w ? 'vertical' : 'square'
  })
</script>

<div class="settings-root">
  <div class="settings-header">
    <span class="title-icon">⚙</span>
    <div>
      <div class="params-title">Settings</div>
      <div class="params-desc">Appearance, defaults, shortcuts, and behavior. Changes preview instantly — Save to persist.</div>
    </div>
  </div>

  <!-- ── Appearance ─────────────────────────────────────────────────── -->
  <section>
    <div class="section-label">Appearance</div>

    <div class="theme-grid-label">Dark</div>
    <div class="theme-grid">
      {#each darkThemes as t}
        <button
          class="theme-swatch"
          class:sel={draft.theme_preset === t.id}
          style="--swatch-bg:{t.vars['--bg']};--swatch-surface:{t.vars['--surface']};--swatch-accent:{t.vars['--orange']};--swatch-text:{t.vars['--text']}"
          onclick={() => (draft.theme_preset = t.id)}
          title={t.label}
        >
          <span class="swatch-preview">
            <span class="swatch-bar"></span>
            <span class="swatch-dot"></span>
          </span>
          <span class="swatch-label">{t.label}</span>
        </button>
      {/each}
    </div>

    <div class="theme-grid-label">Light</div>
    <div class="theme-grid">
      {#each lightThemes as t}
        <button
          class="theme-swatch"
          class:sel={draft.theme_preset === t.id}
          style="--swatch-bg:{t.vars['--bg']};--swatch-surface:{t.vars['--surface']};--swatch-accent:{t.vars['--orange']};--swatch-text:{t.vars['--text']}"
          onclick={() => (draft.theme_preset = t.id)}
          title={t.label}
        >
          <span class="swatch-preview">
            <span class="swatch-bar"></span>
            <span class="swatch-dot"></span>
          </span>
          <span class="swatch-label">{t.label}</span>
        </button>
      {/each}
    </div>

    <div class="field-row">
      <span class="field-label">UI font</span>
      <select class="param-select" bind:value={draft.font_sans}>
        {#each sansFonts as f}
          <option value={f}>{f}</option>
        {/each}
      </select>
    </div>

    <div class="field-row">
      <span class="field-label">Mono font</span>
      <select class="param-select" bind:value={draft.font_mono}>
        {#each monoFonts as f}
          <option value={f}>{f}</option>
        {/each}
      </select>
    </div>

    <div class="field-row">
      <span class="field-label">Layout</span>
      <select class="param-select" bind:value={draft.layout_override}>
        {#each layoutOptions as opt}
          <option value={opt.value}>{opt.label}</option>
        {/each}
      </select>
    </div>

    <div class="field-row">
      <span class="field-label">Wide layout at</span>
      <div class="input-with-hint">
        <input
          class="param-input narrow"
          type="number"
          min="400"
          max="3840"
          step="10"
          bind:value={draft.wide_breakpoint_px}
        />
        <span class="field-hint">px — currently <strong>{resolvedLayout}</strong></span>
      </div>
    </div>
  </section>

  <!-- ── Action defaults ────────────────────────────────────────────── -->
  <section>
    <div class="section-label">Action Defaults</div>

    <div class="field-row">
      <span class="field-label">Bleed (in)</span>
      <input class="param-input narrow" type="number" min="0" step="0.001"
        bind:value={draft.defaults.bleed_inches} />
    </div>
    <div class="field-row">
      <span class="field-label">Trim box bleed (in)</span>
      <input class="param-input narrow" type="number" min="0" step="0.001"
        bind:value={draft.defaults.trim_box_bleed_inches} />
    </div>
    <div class="field-row">
      <span class="field-label">Export format</span>
      <div class="radio-group">
        {#each formatOptions as f}
          <label class="radio-opt" class:sel={draft.defaults.export_format === f}>
            <input type="radio" bind:group={draft.defaults.export_format} value={f} />
            {f.toUpperCase()}
          </label>
        {/each}
      </div>
    </div>
    <div class="field-row">
      <span class="field-label">Export DPI</span>
      <div class="input-with-hint">
        <input class="param-input narrow" type="number" min="36" max="1200" step="1"
          bind:value={draft.defaults.export_dpi} />
        <div class="presets">
          {#each dpiPresets as d}
            <button class="preset-pill" class:sel={draft.defaults.export_dpi === d}
              onclick={() => (draft.defaults.export_dpi = d)}>{d}</button>
          {/each}
        </div>
      </div>
    </div>
    <div class="field-row">
      <span class="field-label">Remap tolerance</span>
      <input class="param-input narrow" type="number" min="0" max="1" step="0.01"
        bind:value={draft.defaults.remap_tolerance} />
    </div>
    <div class="field-row">
      <span class="field-label">Split panel (in)</span>
      <input class="param-input narrow" type="number" min="0.5" step="0.01"
        bind:value={draft.defaults.split_panel_inches} />
    </div>
    <div class="field-row">
      <span class="field-label">Stitch spread (in)</span>
      <input class="param-input narrow" type="number" min="0.5" step="0.01"
        bind:value={draft.defaults.stitch_spread_inches} />
    </div>
    <div class="field-row">
      <span class="field-label">Color intent</span>
      <select class="param-select" bind:value={draft.defaults.color_intent}>
        {#each intentOptions as i}
          <option value={i}>{i}</option>
        {/each}
      </select>
    </div>
  </section>

  <!-- ── Shortcuts ──────────────────────────────────────────────────── -->
  <section>
    <div class="section-label">Shortcuts <span class="section-note">(action keys only)</span></div>
    <div class="shortcut-table">
      {#each ACTION_DEFS as def}
        {@const key = effectiveKey(def.id)}
        {@const conflict = conflictFor(def.id, key)}
        <div class="shortcut-row" class:conflict>
          <span class="shortcut-action">{def.label}</span>
          <input
            class="shortcut-input"
            class:conflict
            type="text"
            maxlength="1"
            value={key}
            placeholder={def.default}
            oninput={(e) => setKey(def.id, e.target.value)}
          />
          {#if conflict}
            <span class="conflict-label">conflicts with {conflict}</span>
          {:else if draft.shortcuts?.[def.id]}
            <button class="reset-key" onclick={() => setKey(def.id, def.default)} title="Reset">↺</button>
          {/if}
        </div>
      {/each}
    </div>
  </section>

  <!-- ── Behavior ───────────────────────────────────────────────────── -->
  <section>
    <div class="section-label">Behavior</div>

    <div class="field-row">
      <span class="field-label">Block files above</span>
      <div class="input-with-hint">
        <input
          class="param-input narrow"
          type="number"
          min="0"
          max="2000"
          step="10"
          bind:value={draft.resource_warn_size_mb}
        />
        <span class="field-hint">
          MB — files larger than this are refused on add (the app can't parse very
          large PDFs yet). <strong>0</strong> disables the limit, but big files will
          likely hang the app.
        </span>
      </div>
    </div>

    <div class="field-row">
      <span class="field-label">Overwrite reminder</span>
      <label class="toggle">
        <input type="checkbox" bind:checked={draft.for_enabled} />
        <span class="toggle-track"></span>
        <span class="toggle-label">{draft.for_enabled ? 'Enabled' : 'Disabled'}</span>
      </label>
    </div>

    <div class="field-row">
      <span class="field-label">Quips</span>
      <label class="toggle">
        <input type="checkbox" bind:checked={draft.quips_enabled} />
        <span class="toggle-track"></span>
        <span class="toggle-label">{draft.quips_enabled ? 'Enabled' : 'Disabled'}</span>
      </label>
    </div>

    {#if draft.quips_enabled}
      <div class="quips-section">
        <div class="quips-header">
          <span class="quips-info">
            {draft.custom_quips?.length
              ? `${draft.custom_quips.length} custom quip${draft.custom_quips.length !== 1 ? 's' : ''}`
              : 'Using built-in quips'}
          </span>
          {#if draft.custom_quips?.length}
            <button class="reset-key" onclick={resetQuips} title="Restore built-in quips">Reset to defaults</button>
          {/if}
        </div>

        {#if draft.custom_quips?.length}
          <div class="quips-list">
            {#each draft.custom_quips as q, i}
              <div class="quip-row">
                <span class="quip-text">{q}</span>
                <button class="quip-remove" onclick={() => removeQuip(i)} title="Remove">✕</button>
              </div>
            {/each}
          </div>
        {/if}

        <div class="quip-add-row">
          <input
            class="param-input quip-input"
            type="text"
            placeholder="Add a quip…"
            bind:value={newQuip}
            onkeydown={(e) => e.key === 'Enter' && addQuip()}
          />
          <button class="btn-add" onclick={addQuip} disabled={!newQuip.trim()}>Add</button>
        </div>
      </div>
    {/if}
  </section>

  <!-- ── Footer ─────────────────────────────────────────────────────── -->
  <div class="footer">
    {#if saveError}
      <span class="error-msg">{saveError}</span>
    {/if}
    <button class="btn-secondary" onclick={reset}>Reset</button>
    <button class="btn-primary" onclick={save}>
      {saved ? '✓ Saved' : 'Save Settings'}
    </button>
  </div>
</div>

<style>
  .settings-root { display: flex; flex-direction: column; gap: 20px; padding-bottom: 8px; }
  .settings-header { display: flex; align-items: center; gap: 10px; }
  .title-icon { font-size: 20px; color: var(--orange); }
  .params-title { font-size: 13px; font-weight: 700; color: var(--text); }
  .params-desc { font-size: 11.5px; color: var(--muted-hi); line-height: 1.5; margin-top: 2px; }

  section { display: flex; flex-direction: column; gap: 10px; }
  .section-label {
    font-size: 10.5px; font-weight: 600; color: var(--muted-hi);
    text-transform: uppercase; letter-spacing: 0.08em;
    border-bottom: 1px solid var(--border); padding-bottom: 5px;
  }
  .section-note { font-size: 9.5px; font-weight: 400; text-transform: none; letter-spacing: 0; color: var(--muted); }

  /* Theme grid */
  .theme-grid-label { font-size: 10px; color: var(--muted); font-weight: 600; text-transform: uppercase; letter-spacing: 0.08em; }
  .theme-grid { display: flex; gap: 8px; flex-wrap: wrap; }
  .theme-swatch {
    display: flex; flex-direction: column; align-items: center; gap: 5px;
    background: var(--swatch-bg); border: 2px solid transparent;
    border-radius: 7px; padding: 7px 10px; cursor: pointer; width: 72px;
    transition: border-color 0.12s;
  }
  .theme-swatch:hover { border-color: var(--border-hi); }
  .theme-swatch.sel { border-color: var(--swatch-accent); }
  .swatch-preview {
    width: 100%; height: 26px; border-radius: 4px;
    background: var(--swatch-surface);
    display: flex; align-items: center; gap: 4px; padding: 0 5px;
  }
  .swatch-bar {
    flex: 1; height: 4px; border-radius: 2px; background: var(--swatch-accent);
  }
  .swatch-dot {
    width: 8px; height: 8px; border-radius: 50%; background: var(--swatch-accent); opacity: 0.6;
  }
  .swatch-label { font-size: 10px; color: var(--swatch-text); font-weight: 500; }

  /* Form fields */
  .field-row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .field-label { font-size: 11.5px; color: var(--text); min-width: 150px; flex-shrink: 0; }
  .field-hint { font-size: 10.5px; color: var(--muted); }
  .field-hint strong { color: var(--orange); }
  .input-with-hint { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }

  .param-input {
    background: var(--panel); border: 1px solid var(--border); border-radius: 5px;
    padding: 6px 10px; color: var(--text); font-family: var(--mono); font-size: 12px; outline: none;
  }
  .param-input.narrow { width: 90px; }
  .param-input:focus { border-color: var(--orange); }

  .param-select {
    background: var(--panel); border: 1px solid var(--border); border-radius: 5px;
    padding: 6px 10px; color: var(--text); font-family: var(--sans); font-size: 12px; outline: none;
  }
  .param-select:focus { border-color: var(--orange); }

  .radio-group { display: flex; gap: 6px; flex-wrap: wrap; }
  .radio-opt {
    display: flex; align-items: center; gap: 5px; font-size: 11.5px; color: var(--muted-hi);
    padding: 5px 10px; border: 1px solid var(--border); border-radius: 5px; cursor: pointer;
  }
  .radio-opt.sel { background: var(--orange-dim); border-color: var(--orange); color: var(--orange-hi); }
  .radio-opt input { display: none; }

  .presets { display: flex; gap: 4px; }
  .preset-pill {
    font-size: 10px; padding: 4px 9px; border-radius: 4px;
    border: 1px solid var(--border); background: var(--panel); color: var(--muted-hi); font-family: var(--mono);
  }
  .preset-pill.sel { background: var(--orange-dim); color: var(--orange-hi); border-color: var(--orange); }

  /* Shortcuts */
  .shortcut-table { display: flex; flex-direction: column; gap: 5px; }
  .shortcut-row { display: flex; align-items: center; gap: 10px; padding: 3px 0; }
  .shortcut-action { font-size: 11.5px; color: var(--text); min-width: 130px; flex-shrink: 0; }
  .shortcut-input {
    width: 36px; text-align: center; background: var(--panel); border: 1px solid var(--border);
    border-radius: 4px; padding: 5px; color: var(--text); font-family: var(--mono); font-size: 13px; outline: none;
  }
  .shortcut-input:focus { border-color: var(--orange); }
  .shortcut-input.conflict { border-color: var(--fail); color: var(--fail); }
  .conflict-label { font-size: 10px; color: var(--fail); }
  .reset-key { font-size: 12px; color: var(--muted); background: none; border: none; cursor: pointer; padding: 2px 5px; }
  .reset-key:hover { color: var(--orange); }

  /* Toggle */
  .toggle { display: flex; align-items: center; gap: 8px; cursor: pointer; }
  .toggle input { display: none; }
  .toggle-track {
    width: 34px; height: 18px; border-radius: 9px; background: var(--border);
    position: relative; transition: background 0.15s; flex-shrink: 0;
  }
  .toggle-track::after {
    content: ''; position: absolute; top: 3px; left: 3px;
    width: 12px; height: 12px; border-radius: 50%;
    background: var(--muted-hi); transition: transform 0.15s, background 0.15s;
  }
  :global(input:checked) + .toggle-track { background: var(--orange-dim); border: 1px solid var(--orange); }
  :global(input:checked) + .toggle-track::after { transform: translateX(16px); background: var(--orange-hi); }
  .toggle-label { font-size: 11.5px; color: var(--muted-hi); }

  /* Quips */
  .quips-section { display: flex; flex-direction: column; gap: 8px; margin-top: 2px; }
  .quips-header { display: flex; align-items: center; gap: 8px; }
  .quips-info { font-size: 11px; color: var(--muted-hi); flex: 1; }
  .quips-list { display: flex; flex-direction: column; gap: 4px; max-height: 180px; overflow-y: auto; }
  .quip-row {
    display: flex; align-items: center; gap: 8px;
    background: var(--panel); border: 1px solid var(--border); border-radius: 5px;
    padding: 5px 10px;
  }
  .quip-text { flex: 1; font-size: 11.5px; color: var(--text); }
  .quip-remove { font-size: 10px; color: var(--muted); background: none; border: none; cursor: pointer; padding: 2px 4px; }
  .quip-remove:hover { color: var(--fail); }
  .quip-add-row { display: flex; gap: 6px; }
  .quip-input { flex: 1; font-family: var(--sans) !important; }
  .btn-add {
    padding: 6px 14px; background: var(--panel); border: 1px solid var(--border);
    border-radius: 5px; color: var(--muted-hi); font-size: 12px; cursor: pointer;
  }
  .btn-add:hover:not(:disabled) { border-color: var(--orange); color: var(--orange-hi); }
  .btn-add:disabled { opacity: 0.4; cursor: not-allowed; }

  /* Footer */
  .footer {
    display: flex; align-items: center; gap: 8px; justify-content: flex-end;
    padding-top: 4px; border-top: 1px solid var(--border);
  }
  .error-msg { font-size: 11px; color: var(--fail); flex: 1; }
  .btn-primary {
    padding: 7px 18px; background: var(--orange); border: none; border-radius: 5px;
    color: #fff; font-size: 12px; font-weight: 600; cursor: pointer;
  }
  .btn-primary:hover { opacity: 0.9; }
  .btn-secondary {
    padding: 7px 14px; background: var(--panel); border: 1px solid var(--border);
    border-radius: 5px; color: var(--muted-hi); font-size: 12px; cursor: pointer;
  }
  .btn-secondary:hover { border-color: var(--border-hi); color: var(--text); }
</style>
