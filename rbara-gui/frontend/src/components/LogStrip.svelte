<script>
  import { useAppState } from '../lib/context.js'
  const app = useAppState()
  let last = $derived(app.actionLog[0])
</script>

<div class="strip">
  {#if last}
    <span class="badge" class:ok={last.ok} class:fail={!last.ok}
      >{last.ok ? 'OK' : 'FAIL'}</span
    >
    <span class="action">{last.action}</span>
    <span class="msg">{last.message}</span>
    <span class="time">{last.timestamp}</span>
  {:else}
    <span class="idle">No activity yet</span>
  {/if}
</div>

<style>
  .strip {
    background: var(--surface);
    border-top: 1px solid var(--border);
    padding: 6px 12px;
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--mono);
    font-size: 11px;
    flex-shrink: 0;
    overflow: hidden;
    white-space: nowrap;
  }
  .badge {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 3px;
    flex-shrink: 0;
  }
  .badge.ok {
    background: color-mix(in srgb, var(--ok) 12%, transparent);
    color: var(--ok);
    border: 1px solid color-mix(in srgb, var(--ok) 40%, transparent);
  }
  .badge.fail {
    background: color-mix(in srgb, var(--fail) 12%, transparent);
    color: var(--fail);
    border: 1px solid color-mix(in srgb, var(--fail) 40%, transparent);
  }
  .action {
    color: var(--text);
    flex-shrink: 0;
  }
  .msg {
    color: var(--muted-hi);
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
  }
  .time {
    color: var(--muted);
    margin-left: auto;
    flex-shrink: 0;
  }
  .idle {
    color: var(--muted);
    font-style: italic;
  }
</style>
