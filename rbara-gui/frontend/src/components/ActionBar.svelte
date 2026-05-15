<script>
  import { useAppState } from '../lib/context.js';
  const app = useAppState();

  const trimActions = [
    { id: 'trim',       icon: '✂', label: 'Trim Marks',  key: 't' },
    { id: 'addtrimbox', icon: '⊞', label: 'Add Trim Box', key: 'b' },
  ];

  const mainActions = [
    { id: 'resize', icon: '⊡', label: 'Resize', key: 'r' },
    { id: 'export', icon: '⇲', label: 'Export', key: 'x' },
  ];

  const pagesActions = [
    { id: 'splitpages',   icon: '⧉', label: 'Split Pages',   key: 'p' },
    { id: 'extractpages', icon: '⊟', label: 'Extract Pages', key: 'e' },
  ];

  const colorActions = [
    { id: 'remap',      icon: '⬡', label: 'Remap',   key: 'm' },
    { id: 'colorspace', icon: '◈', label: 'Convert',  key: 'c' },
    { id: 'spots',      icon: '✦', label: 'Spots',    key: 's' },
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

<!-- Grid order: Trim ▾ | Resize | Export | Pages ▾ | Color ▾ | Output -->
<div class="action-bar">
  <div class="ab-color-wrap">
    <button
      class="ab-btn"
      class:active={isTrimActive}
      onclick={() => (trimMenuOpen = !trimMenuOpen)}
    >
      <span class="ab-icon">{activeTrimAction?.icon ?? '✂'}</span>
      <span class="ab-label">Trim ▾</span>
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
      class="ab-btn"
      class:active={app.activeAction === a.id}
      onclick={() => (app.activeAction = a.id)}
    >
      <span class="ab-icon">{a.icon}</span>
      <span class="ab-label">{a.label}</span>
      <span class="hk">{a.key}</span>
    </button>
  {/each}

  <div class="ab-color-wrap">
    <button
      class="ab-btn"
      class:active={isPagesActive}
      onclick={() => (pagesMenuOpen = !pagesMenuOpen)}
    >
      <span class="ab-icon">{activePagesAction?.icon ?? '◫'}</span>
      <span class="ab-label">Pages ▾</span>
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

  <div class="ab-color-wrap">
    <button
      class="ab-btn"
      class:active={isColorActive}
      onclick={() => (colorMenuOpen = !colorMenuOpen)}
    >
      <span class="ab-icon">{activeColorAction?.icon ?? '⬡'}</span>
      <span class="ab-label">Color ▾</span>
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

  <button
    class="ab-btn"
    class:active={app.activeAction === 'output'}
    onclick={() => (app.activeAction = 'output')}
  >
    <span class="ab-icon">⊘</span>
    <span class="ab-label">Output</span>
    <span class="hk">/</span>
  </button>
</div>

<style>
  .action-bar {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .ab-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding: 8px 4px;
    background: transparent;
    border: none;
    border-right: 1px solid var(--border);
    color: var(--muted-hi);
    font-size: 11px;
    width: 100%;
  }
  .ab-btn:last-child { border-right: none; }
  .ab-btn:hover { background: var(--panel); color: var(--text); }
  .ab-btn.active { color: var(--orange-hi); background: var(--orange-dim); }
  .ab-icon { font-size: 18px; }
  .ab-label { font-size: 11px; }

  .ab-color-wrap { position: relative; border-right: 1px solid var(--border); }
  .ab-color-wrap .ab-btn { border-right: none; }

  .overlay {
    position: fixed;
    inset: 0;
    z-index: 50;
  }
  .color-menu {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
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
    padding: 8px 10px;
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--border);
    color: var(--muted-hi);
    font-size: 11.5px;
    text-align: left;
    white-space: nowrap;
  }
  .cm-item:last-child { border-bottom: none; }
  .cm-item:hover { background: var(--panel); color: var(--text); }
  .cm-item.active { color: var(--orange-hi); background: var(--orange-dim); }
  .cm-icon { font-size: 13px; width: 16px; text-align: center; flex-shrink: 0; }
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
