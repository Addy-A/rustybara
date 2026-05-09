<script>
  import { useAppState } from '../lib/context.js';
  const app = useAppState();

  const actions = [
    { id: 'trim', icon: '✂', label: 'Trim', key: 't' },
    { id: 'resize', icon: '⊡', label: 'Resize', key: 'r' },
    { id: 'export', icon: '⇲', label: 'Export', key: 'x' },
    { id: 'remap', icon: '⬡', label: 'Remap', key: 'm' },
    { id: 'output', icon: '⊘', label: 'Output', key: '/' },
  ];
</script>

<div class="action-bar">
  {#each actions as a (a.id)}
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
</div>

<style>
  .action-bar {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
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
  }
  .ab-btn:last-child { border-right: none; }
  .ab-btn:hover { background: var(--panel); color: var(--text); }
  .ab-btn.active { color: var(--orange-hi); background: var(--orange-dim); }
  .ab-icon { font-size: 18px; }
  .ab-label { font-size: 11px; }
</style>
