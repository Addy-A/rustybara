<script>
  import { useAppState } from '../lib/context.js';
  const app = useAppState();

  const actions = [
    { id: 'trim', icon: '✂', label: 'Trim Marks', key: 't' },
    { id: 'resize', icon: '⊡', label: 'Resize to Bleed', key: 'r' },
    { id: 'export', icon: '⇲', label: 'Export Images', key: 'x' },
    { id: 'remap', icon: '⬡', label: 'Remap Colors', key: 'm' },
  ];
</script>

<div class="actions-pane">
  <div class="pane-label">Actions</div>
  {#each actions as a (a.id)}
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
  .actions-footer {
    margin-top: auto;
    border-top: 1px solid var(--border);
  }
</style>
