<script>
  import { useAppState } from '../lib/context.js';
  const app = useAppState();

  const mainActions = [
    { id: 'trim', icon: '✂', label: 'Trim Marks', key: 't' },
    { id: 'resize', icon: '⊡', label: 'Resize to Bleed', key: 'r' },
    { id: 'export', icon: '⇲', label: 'Export Images', key: 'x' },
  ];

  const pagesActions = [
    { id: 'addtrimbox',   icon: '⊞', label: 'Add Trim Box',    key: 'b' },
    { id: 'splitpages',   icon: '⧉', label: 'Split Pages',     key: 'p' },
    { id: 'extractpages', icon: '⊟', label: 'Extract Pages',   key: 'e' },
  ];

  const colorActions = [
    { id: 'remap',      icon: '⬡', label: 'Remap Colors',       key: 'm' },
    { id: 'colorspace', icon: '◈', label: 'Convert Color Space', key: 'c' },
    { id: 'spots',      icon: '✦', label: 'Flatten Spot Colors', key: 's' },
  ];

  const pagesIds = new Set(['addtrimbox', 'splitpages', 'extractpages']);
  const colorIds = new Set(['remap', 'colorspace', 'spots']);

  let pagesExpanded = $state(pagesIds.has(app.activeAction));
  let colorExpanded = $state(colorIds.has(app.activeAction));
  let isPagesActive = $derived(pagesIds.has(app.activeAction));
  let isColorActive = $derived(colorIds.has(app.activeAction));

  $effect(() => {
    if (pagesIds.has(app.activeAction)) pagesExpanded = true;
    if (colorIds.has(app.activeAction)) colorExpanded = true;
  });
</script>

<div class="actions-pane">
  <div class="pane-label">Actions</div>

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
    onclick={() => (pagesExpanded = !pagesExpanded)}
    role="button"
    tabindex="0"
  >
    <span class="ai-icon">◫</span>
    <span class="ai-label">Pages</span>
    <span class="chevron">{pagesExpanded ? '▾' : '▸'}</span>
  </div>

  {#if pagesExpanded}
    {#each pagesActions as a (a.id)}
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
    class:active={isColorActive}
    onclick={() => (colorExpanded = !colorExpanded)}
    role="button"
    tabindex="0"
  >
    <span class="ai-icon">⬡</span>
    <span class="ai-label">Color</span>
    <span class="chevron">{colorExpanded ? '▾' : '▸'}</span>
  </div>

  {#if colorExpanded}
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

  <div class="actions-footer">
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
    width: 210px;
    flex-shrink: 0;
    background: var(--surface);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
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
  .action-item:hover { background: var(--panel); color: var(--text); }
  .action-item.active {
    background: var(--orange-dim);
    color: var(--orange-hi);
    border-left-color: var(--orange);
  }
  .action-item.muted { color: var(--muted); font-size: 12px; }
  .action-item.nested { padding-left: 28px; font-size: 12px; }
  .ai-icon { font-size: 15px; width: 18px; text-align: center; flex-shrink: 0; }
  .ai-label { flex: 1; }
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
  .group-header:hover { background: var(--panel); color: var(--text); }
  .group-header.active {
    color: var(--orange-hi);
    border-left-color: var(--orange);
  }
  .chevron { margin-left: auto; font-size: 10px; color: var(--muted); }

  .actions-footer {
    margin-top: auto;
    border-top: 1px solid var(--border);
  }
</style>
