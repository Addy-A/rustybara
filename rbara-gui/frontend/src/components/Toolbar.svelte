<script>
  import { useAppState } from '../lib/context.js';
  const app = useAppState();
</script>

<div class="toolbar">
  <button class="tb-btn primary" onclick={() => app.addFiles()} disabled={app.processing}>
    ＋ Add Files <span class="hk">f</span>
  </button>
  <div class="tb-sep"></div>
  <button class="tb-btn" onclick={() => (app.activeAction = 'output')}>
    Output Path <span class="hk">/</span>
  </button>
  <div class="tb-sep"></div>
  <button class="tb-btn danger" onclick={() => app.clearAll()} disabled={app.files.length === 0}>
    Clear All
  </button>
  <div class="tb-right">
    <button class="tb-btn" style="color:var(--muted)" onclick={() => (app.helpVisible = true)}>
      ? Help
    </button>
  </div>
</div>

<style>
  .toolbar {
    height: 44px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    padding: 0 14px;
    gap: 6px;
    flex-shrink: 0;
  }
  .tb-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 12px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--muted-hi);
    font-size: 12px;
    font-family: var(--sans);
    transition: 0.12s;
    white-space: nowrap;
  }
  .tb-btn:hover:not(:disabled) {
    border-color: var(--border-hi);
    color: var(--text);
  }
  .tb-btn.primary {
    background: var(--orange-dim);
    border-color: var(--orange);
    color: var(--orange-hi);
  }
  .tb-btn.primary:hover:not(:disabled) { background: #e8680733; }
  .tb-btn.danger {
    border-color: #5a2020;
    color: #f87171;
    background: #2a0f0f;
  }
  .tb-sep {
    width: 1px;
    height: 22px;
    background: var(--border);
    margin: 0 4px;
  }
  .tb-right { margin-left: auto; }
</style>
