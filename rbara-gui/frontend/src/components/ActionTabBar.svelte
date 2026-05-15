<script>
  import { useAppState } from '../lib/context.js';
  const app = useAppState();

  const trimActions = [
    { id: 'trim',       icon: '✂', label: 'Trim Marks',  key: 't' },
    { id: 'addtrimbox', icon: '⊞', label: 'Add Trim Box', key: 'b' },
  ];

  const mainActions = [
    { id: 'resize', icon: '⊡', label: 'Resize to Bleed', key: 'r' },
    { id: 'export', icon: '⇲', label: 'Export Images',   key: 'x' },
    { id: 'output', icon: '⊘', label: 'Output',          key: '/' },
  ];

  const pagesActions = [
    { id: 'splitpages',   icon: '⧉', label: 'Split Pages',   key: 'p' },
    { id: 'extractpages', icon: '⊟', label: 'Extract Pages', key: 'e' },
  ];

  const colorActions = [
    { id: 'remap',      icon: '⬡', label: 'Remap Colors',       key: 'm' },
    { id: 'colorspace', icon: '◈', label: 'Convert Color Space', key: 'c' },
    { id: 'spots',      icon: '✦', label: 'Flatten Spot Colors', key: 's' },
  ];

  const trimIds  = new Set(['trim', 'addtrimbox']);
  const pagesIds = new Set(['splitpages', 'extractpages']);
  const colorIds = new Set(['remap', 'colorspace', 'spots']);

  let trimMenuOpen  = $state(false);
  let pagesMenuOpen = $state(false);
  let colorMenuOpen = $state(false);

  let isTrimActive  = $derived(trimIds.has(app.activeAction));
  let isPagesActive = $derived(pagesIds.has(app.activeAction));
  let isColorActive = $derived(colorIds.has(app.activeAction));
  let activeTrimAction  = $derived(trimActions.find(a => a.id === app.activeAction));
  let activePagesAction = $derived(pagesActions.find(a => a.id === app.activeAction));
  let activeColorAction = $derived(colorActions.find(a => a.id === app.activeAction));

  function selectTrim(id)  { app.activeAction = id; trimMenuOpen  = false; }
  function selectPages(id) { app.activeAction = id; pagesMenuOpen = false; }
  function selectColor(id) { app.activeAction = id; colorMenuOpen = false; }
</script>

<div class="tab-bar">
  <div class="tab-color-wrap">
    <button
      class="tab"
      class:active={isTrimActive}
      onclick={() => (trimMenuOpen = !trimMenuOpen)}
    >
      <span class="t-icon">{activeTrimAction?.icon ?? '✂'}</span>
      <span class="t-label">Trim ▾</span>
      <span class="hk">{activeTrimAction?.key ?? '…'}</span>
    </button>

    {#if trimMenuOpen}
      <div class="overlay" onclick={() => (trimMenuOpen = false)} role="presentation"></div>
      <div class="color-menu">
        {#each trimActions as a (a.id)}
          <button
            class="cm-item"
            class:active={app.activeAction === a.id}
            onclick={() => selectTrim(a.id)}
          >
            <span class="cm-icon">{a.icon}</span>
            <span class="cm-label">{a.label}</span>
            <span class="cm-key">{a.key}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  {#each mainActions as a (a.id)}
    <button
      class="tab"
      class:active={app.activeAction === a.id}
      onclick={() => (app.activeAction = a.id)}
    >
      <span class="t-icon">{a.icon}</span>
      <span class="t-label">{a.label}</span>
      <span class="hk">{a.key}</span>
    </button>
  {/each}

  <div class="tab-color-wrap">
    <button
      class="tab"
      class:active={isPagesActive}
      onclick={() => (pagesMenuOpen = !pagesMenuOpen)}
    >
      <span class="t-icon">{activePagesAction?.icon ?? '◫'}</span>
      <span class="t-label">Pages ▾</span>
      <span class="hk">{activePagesAction?.key ?? '…'}</span>
    </button>

    {#if pagesMenuOpen}
      <div class="overlay" onclick={() => (pagesMenuOpen = false)} role="presentation"></div>
      <div class="color-menu">
        {#each pagesActions as a (a.id)}
          <button
            class="cm-item"
            class:active={app.activeAction === a.id}
            onclick={() => selectPages(a.id)}
          >
            <span class="cm-icon">{a.icon}</span>
            <span class="cm-label">{a.label}</span>
            <span class="cm-key">{a.key}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <div class="tab-color-wrap">
    <button
      class="tab"
      class:active={isColorActive}
      onclick={() => (colorMenuOpen = !colorMenuOpen)}
    >
      <span class="t-icon">{activeColorAction?.icon ?? '⬡'}</span>
      <span class="t-label">Color ▾</span>
      <span class="hk">{activeColorAction?.key ?? '…'}</span>
    </button>

    {#if colorMenuOpen}
      <div class="overlay" onclick={() => (colorMenuOpen = false)} role="presentation"></div>
      <div class="color-menu">
        {#each colorActions as a (a.id)}
          <button
            class="cm-item"
            class:active={app.activeAction === a.id}
            onclick={() => selectColor(a.id)}
          >
            <span class="cm-icon">{a.icon}</span>
            <span class="cm-label">{a.label}</span>
            <span class="cm-key">{a.key}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .tab-bar {
    display: flex;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    overflow-x: auto;
  }
  .tab {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 10px 8px;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--muted-hi);
    font-size: 12px;
    white-space: nowrap;
  }
  .tab:hover { color: var(--text); background: var(--panel); }
  .tab.active {
    color: var(--orange-hi);
    border-bottom-color: var(--orange);
    background: var(--orange-dim);
  }
  .t-icon { font-size: 14px; }

  .tab-color-wrap { position: relative; flex: 1; display: flex; }
  .tab-color-wrap .tab { width: 100%; }

  .overlay {
    position: fixed;
    inset: 0;
    z-index: 50;
  }
  .color-menu {
    position: absolute;
    top: 100%;
    left: 0;
    min-width: 190px;
    z-index: 51;
    background: var(--surface);
    border: 1px solid var(--border);
    border-top: none;
    box-shadow: 0 4px 12px rgba(0,0,0,0.3);
  }
  .cm-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 9px 12px;
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--border);
    color: var(--muted-hi);
    font-size: 12px;
    text-align: left;
    white-space: nowrap;
  }
  .cm-item:last-child { border-bottom: none; }
  .cm-item:hover { background: var(--panel); color: var(--text); }
  .cm-item.active { color: var(--orange-hi); background: var(--orange-dim); }
  .cm-icon { font-size: 14px; width: 18px; text-align: center; flex-shrink: 0; }
  .cm-label { flex: 1; }
  .cm-key {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--muted);
    background: var(--bg);
    border: 1px solid var(--border);
    padding: 0 3px;
    border-radius: 3px;
    opacity: 0.7;
  }
</style>
