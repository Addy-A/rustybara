<script>
  import { useAppState } from '../lib/context.js'
  const app = useAppState()
  let open = $state(false)
</script>

<div class="drawer" class:open>
  <button class="handle" onclick={() => (open = !open)}>
    <span>Activity Log ({app.actionLog.length})</span>
    <span class="arrow" class:open>▴</span>
  </button>
  {#if open}
    <div class="list">
      {#each app.actionLog as entry, i (i)}
        <div class="entry">
          <span class="badge" class:ok={entry.ok} class:fail={!entry.ok}>
            {entry.ok ? 'OK' : 'FAIL'}
          </span>
          <span class="action">{entry.action}</span>
          <span class="time">{entry.timestamp}</span>
          <div class="detail" class:err={!entry.ok}>{entry.message}</div>
        </div>
      {:else}
        <div class="empty">No activity yet</div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .drawer {
    background: var(--surface);
    border-top: 1px solid var(--border);
    flex-shrink: 0;
    max-height: 50vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .handle {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 14px;
    background: transparent;
    border: none;
    color: var(--muted-hi);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-weight: 700;
  }
  .arrow {
    color: var(--orange);
    transition: transform 0.15s;
  }
  .arrow.open {
    transform: rotate(180deg);
  }
  .list {
    overflow-y: auto;
    border-top: 1px solid var(--border);
  }
  .entry {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    font-family: var(--mono);
    font-size: 11px;
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 6px;
    align-items: center;
  }
  .badge {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 3px;
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
  }
  .time {
    color: var(--muted);
    font-size: 10px;
  }
  .detail {
    grid-column: 1 / -1;
    color: var(--muted);
    font-size: 10.5px;
  }
  .detail.err {
    color: var(--fail);
  }
  .empty {
    padding: 16px;
    color: var(--muted);
    font-size: 11px;
    text-align: center;
    font-style: italic;
  }
</style>
