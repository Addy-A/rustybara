<script>
  import { useAppState } from '../lib/context.js';
  const app = useAppState();
  let { label = 'Run', icon = '▶', disabled = false } = $props();

  let running = $derived(app.processing);
  let isDisabled = $derived(disabled || running || app.scopedCount === 0);
</script>

<button class="run-btn" disabled={isDisabled} onclick={() => app.runAction()}>
  <span>{icon}</span>
  <span>{running ? 'Running…' : `${label} (${app.scopedCount})`}</span>
  <span class="run-key">Enter</span>
</button>

<style>
  .run-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 10px 20px;
    background: var(--orange);
    border: none;
    border-radius: 7px;
    color: #fff;
    font-size: 13px;
    font-weight: 700;
    font-family: var(--sans);
    transition: 0.12s;
    align-self: flex-start;
  }
  .run-btn:hover:not(:disabled) { background: var(--orange-hi); }
  .run-key {
    font-size: 11px;
    background: #ffffff33;
    padding: 1px 6px;
    border-radius: 4px;
    font-family: var(--mono);
    font-weight: 400;
  }
</style>
