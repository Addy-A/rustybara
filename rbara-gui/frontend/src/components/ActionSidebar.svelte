<script>
  import { useAppState } from '../lib/context.js'
  const app = useAppState()

  const MIN_W = 150
  const MAX_W = 320

  // Bootstrap from localStorage so the sidebar is the right size before
  // settings loads asynchronously. After load, settings is the source of truth.
  let width = $state(Math.max(MIN_W, Math.min(MAX_W,
    app.settings?.sidebar_width
      ?? parseInt(localStorage.getItem('rbara-sidebar-left-width') ?? '210', 10)
  )))

  // Sync width if settings load after initial render with a different value.
  $effect(() => {
    if (app.settings?.sidebar_width) {
      width = Math.max(MIN_W, Math.min(MAX_W, app.settings.sidebar_width))
    }
  })

  let dragging = false
  let startX = 0
  let startWidth = 0

  function onHandleDown(e) {
    dragging = true
    startX = e.clientX
    startWidth = width
    e.preventDefault()
  }

  function onMouseMove(e) {
    if (!dragging) return
    width = Math.max(MIN_W, Math.min(MAX_W, startWidth + (e.clientX - startX)))
  }

  function onMouseUp() {
    if (!dragging) return
    dragging = false
    if (app.settings) {
      app.saveSettings({ ...app.settings, sidebar_width: width }).catch(() => {})
    }
  }

  const trimActions = [
    { id: 'trim', icon: '✂', label: 'Trim Marks', key: 't' },
    { id: 'addtrimbox', icon: '⊞', label: 'Add Trim Box', key: 'b' },
  ]

  const boxesActions = [
    { id: 'resize',      icon: '⊡', label: 'Resize to Bleed', key: 'r'  },
    { id: 'setmediabox', icon: '▭', label: 'Set Media Box',   key: '⇧m' },
  ]

  // Orphaned actions that don't belong to a category live flat in the list.
  const mainActions = [
    { id: 'export',      icon: '⇲', label: 'Export Images',   key: 'x'  },
  ]

  const miscActions = [
    { id: 'rotate',      icon: '⟳', label: 'Rotate PDF',   key: '⇧r' },
    { id: 'outlinetext', icon: '⊤', label: 'Outline Text', key: '⇧t' },
  ]

  const pagesActions = [
    { id: 'splitpages', icon: '⧉', label: 'Split Pages', key: 'p' },
    { id: 'stitchpages', icon: '⧈', label: 'Stitch Pages', key: 'g', exp: true },
    { id: 'extractpages', icon: '⊟', label: 'Extract Pages', key: 'e' },
  ]

  const colorActions = [
    { id: 'remap', icon: '⬡', label: 'Remap Colors', key: 'm' },
    { id: 'colorspace', icon: '◈', label: 'Convert Color Space', key: 'c' },
    { id: 'spots', icon: '✦', label: 'Flatten Spot Colors', key: 's' },
  ]

  const trimIds = new Set(['trim', 'addtrimbox'])
  const boxesIds = new Set(['resize', 'setmediabox'])
  const miscIds = new Set(['rotate', 'outlinetext'])
  const pagesIds = new Set(['splitpages', 'stitchpages', 'extractpages'])
  const colorIds = new Set(['remap', 'colorspace', 'spots'])

  let isTrimActive = $derived(trimIds.has(app.activeAction))
  let isBoxesActive = $derived(boxesIds.has(app.activeAction))
  let isMiscActive = $derived(miscIds.has(app.activeAction))
  let isPagesActive = $derived(pagesIds.has(app.activeAction))
  let isColorActive = $derived(colorIds.has(app.activeAction))
</script>

<svelte:window onmousemove={onMouseMove} onmouseup={onMouseUp} />

<div class="actions-pane" style="width: {width}px">
  <div
    class="resize-handle"
    onmousedown={onHandleDown}
    role="separator"
    aria-orientation="vertical"
  ></div>
  <div class="pane-label">Actions</div>

  <div
    class="group-header"
    class:active={isTrimActive}
    onclick={() => (app.trimExpanded = !app.trimExpanded)}
    role="button"
    tabindex="0"
  >
    <span class="ai-icon">✂</span>
    <span class="ai-label">Trim</span>
    <span class="chevron">{app.trimExpanded ? '▾' : '▸'}</span>
  </div>

  {#if app.trimExpanded}
    {#each trimActions as a (a.id)}
      <div
        class="action-item nested"
        class:active={app.activeAction === a.id}
        onclick={() => (app.activeAction = a.id)}
        role="button"
        tabindex="0"
      >
        <span class="ai-icon">{a.icon}</span>
        <span class="ai-label">{a.label}</span>
        <span class="ai-key">{a.key}</span>
      </div>
    {/each}
  {/if}

  <div
    class="group-header"
    class:active={isBoxesActive}
    onclick={() => (app.boxesExpanded = !app.boxesExpanded)}
    role="button"
    tabindex="0"
  >
    <span class="ai-icon">⬚</span>
    <span class="ai-label">Boxes</span>
    <span class="chevron">{app.boxesExpanded ? '▾' : '▸'}</span>
  </div>

  {#if app.boxesExpanded}
    {#each boxesActions as a (a.id)}
      <div
        class="action-item nested"
        class:active={app.activeAction === a.id}
        onclick={() => (app.activeAction = a.id)}
        role="button"
        tabindex="0"
      >
        <span class="ai-icon">{a.icon}</span>
        <span class="ai-label">{a.label}</span>
        <span class="ai-key">{a.key}</span>
      </div>
    {/each}
  {/if}

  {#each mainActions as a (a.id)}
    <div
      class="action-item"
      class:active={app.activeAction === a.id}
      onclick={() => (app.activeAction = a.id)}
      role="button"
      tabindex="0"
    >
      <span class="ai-icon">{a.icon}</span>
      <span class="ai-label">{a.label}</span>
      <span class="ai-key">{a.key}</span>
    </div>
  {/each}

  <div
    class="group-header"
    class:active={isPagesActive}
    onclick={() => (app.pagesExpanded = !app.pagesExpanded)}
    role="button"
    tabindex="0"
  >
    <span class="ai-icon">◫</span>
    <span class="ai-label">Pages</span>
    <span class="chevron">{app.pagesExpanded ? '▾' : '▸'}</span>
  </div>

  {#if app.pagesExpanded}
    {#each pagesActions as a (a.id)}
      <div
        class="action-item nested"
        class:active={app.activeAction === a.id}
        onclick={() => (app.activeAction = a.id)}
        role="button"
        tabindex="0"
      >
        <span class="ai-icon">{a.icon}</span>
        <span class="ai-label">{a.label}{#if a.exp}&nbsp;<span class="exp-badge">exp</span>{/if}</span>
        <span class="ai-key">{a.key}</span>
      </div>
    {/each}
  {/if}

  <div
    class="group-header"
    class:active={isColorActive}
    onclick={() => (app.colorExpanded = !app.colorExpanded)}
    role="button"
    tabindex="0"
  >
    <span class="ai-icon">⬡</span>
    <span class="ai-label">Color</span>
    <span class="chevron">{app.colorExpanded ? '▾' : '▸'}</span>
  </div>

  {#if app.colorExpanded}
    {#each colorActions as a (a.id)}
      <div
        class="action-item nested"
        class:active={app.activeAction === a.id}
        onclick={() => (app.activeAction = a.id)}
        role="button"
        tabindex="0"
      >
        <span class="ai-icon">{a.icon}</span>
        <span class="ai-label">{a.label}</span>
        <span class="ai-key">{a.key}</span>
      </div>
    {/each}
  {/if}

  <div
    class="group-header"
    class:active={isMiscActive}
    onclick={() => (app.miscExpanded = !app.miscExpanded)}
    role="button"
    tabindex="0"
  >
    <span class="ai-icon">⋯</span>
    <span class="ai-label">Miscellaneous</span>
    <span class="chevron">{app.miscExpanded ? '▾' : '▸'}</span>
  </div>

  {#if app.miscExpanded}
    {#each miscActions as a (a.id)}
      <div
        class="action-item nested"
        class:active={app.activeAction === a.id}
        onclick={() => (app.activeAction = a.id)}
        role="button"
        tabindex="0"
      >
        <span class="ai-icon">{a.icon}</span>
        <span class="ai-label">{a.label}</span>
        <span class="ai-key">{a.key}</span>
      </div>
    {/each}
  {/if}

  <div class="actions-footer">
    <div
      class="action-item view-btn"
      class:disabled={!app.activeFileObj}
      onclick={() => app.viewInRbv(app.activeFileObj)}
      role="button"
      tabindex="0"
      title="Open active file in rbv viewer"
    >
      <span class="ai-icon">⊙</span>
      <span class="ai-label">View in rbv</span>
      <span class="ai-key">v</span>
    </div>
    <div
      class="action-item muted"
      class:active={app.activeAction === 'output'}
      onclick={() => (app.activeAction = 'output')}
      role="button"
      tabindex="0"
    >
      <span class="ai-icon">⊘</span>
      <span class="ai-label">Output Path</span>
      <span class="ai-key">/</span>
    </div>
  </div>
</div>

<style>
  .actions-pane {
    flex-shrink: 0;
    background: var(--surface);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    position: relative;
  }
  .resize-handle {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    width: 4px;
    cursor: ew-resize;
    z-index: 10;
    transition: background 0.15s;
  }
  .resize-handle:hover {
    background: color-mix(in srgb, var(--orange) 40%, transparent);
  }
  .action-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    cursor: pointer;
    border-bottom: 1px solid var(--border);
    color: var(--muted-hi);
    font-size: 12.5px;
    transition: 0.1s;
    border-left: 2px solid transparent;
  }
  .action-item:hover {
    background: var(--panel);
    color: var(--text);
  }
  .action-item.active {
    background: var(--orange-dim);
    color: var(--orange-hi);
    border-left-color: var(--orange);
  }
  .action-item.muted {
    color: var(--muted);
    font-size: 12px;
  }
  .action-item.view-btn {
    border-top: 1px solid var(--border);
  }
  .action-item.view-btn.disabled {
    opacity: 0.35;
    pointer-events: none;
  }
  .action-item.nested {
    padding-left: 28px;
    font-size: 12px;
  }
  .ai-icon {
    font-size: 15px;
    width: 18px;
    text-align: center;
    flex-shrink: 0;
  }
  .ai-label {
    flex: 1;
  }
  .ai-key {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--muted);
    background: var(--bg);
    border: 1px solid var(--border);
    padding: 0 4px;
    border-radius: 3px;
    opacity: 0.7;
  }
  .exp-badge {
    font-size: 7.5px;
    font-family: var(--mono);
    color: var(--muted);
    border: 1px solid var(--border);
    border-radius: 2px;
    padding: 0 3px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    vertical-align: middle;
  }

  .group-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    cursor: pointer;
    border-bottom: 1px solid var(--border);
    border-left: 2px solid transparent;
    color: var(--muted-hi);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    transition: 0.1s;
    user-select: none;
  }
  .group-header:hover {
    background: var(--panel);
    color: var(--text);
  }
  .group-header.active {
    color: var(--orange-hi);
    border-left-color: var(--orange);
  }
  .chevron {
    margin-left: auto;
    font-size: 10px;
    color: var(--muted);
  }

  .actions-footer {
    margin-top: auto;
    border-top: 1px solid var(--border);
  }

  .pane-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--muted);
    padding: 8px 12px 6px;
    border-bottom: 1px solid var(--border);
  }
</style>
