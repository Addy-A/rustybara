<script>
  import { useAppState } from '../lib/context.js';
  const app = useAppState();

  const actions = [
    { id: 'trim', icon: '✂', label: 'Trim Marks', key: 't' },
    { id: 'resize', icon: '⊡', label: 'Resize to Bleed', key: 'r' },
    { id: 'export', icon: '⇲', label: 'Export Images', key: 'x' },
    { id: 'remap', icon: '⬡', label: 'Remap Colors', key: 'm' },
    { id: 'output', icon: '⊘', label: 'Output', key: '/' },
  ];
</script>

<div class="tab-bar">
  {#each actions as a (a.id)}
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
</style>
