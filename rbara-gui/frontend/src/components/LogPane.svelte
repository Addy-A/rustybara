<script>
  import { useAppState } from '../lib/context.js';
  const app = useAppState();

  function badgeClass(entry) {
    if (entry.ok) return 'log-ok';
    return 'log-fail';
  }
  function badgeText(entry) {
    return entry.ok ? 'OK' : 'FAIL';
  }
</script>

<div class="log-pane">
  <div class="pane-label">Activity Log</div>
  <div class="log-list">
    {#each app.actionLog as entry, i (i)}
      <div class="log-entry">
        <div class="log-header-row">
          <span class="log-badge {badgeClass(entry)}">{badgeText(entry)}</span>
          <span class="log-action">{entry.action}</span>
          <span class="log-time">{entry.timestamp}</span>
        </div>
        <div class="log-detail" class:err={!entry.ok}>{entry.message}</div>
      </div>
    {:else}
      <div class="log-empty">No activity yet</div>
    {/each}
  </div>
  <div class="output-row">
    <div class="output-row-label">Output Directory</div>
    <div class="output-path-value">
      {app.overwrite ? '(overwriting source)' : (app.outputDir ?? '~/source folder')}
    </div>
  </div>
</div>

<style>
  .log-pane {
    width: 255px;
    flex-shrink: 0;
    background: var(--surface);
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .log-list {
    flex: 1;
    overflow-y: auto;
  }
  .log-entry {
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    font-family: var(--mono);
    font-size: 11px;
  }
  .log-entry:hover { background: var(--panel); }
  .log-header-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 3px;
  }
  .log-badge {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 3px;
    letter-spacing: 0.06em;
  }
  .log-ok { background: #022c1e; color: #4ade80; border: 1px solid #065f46; }
  .log-fail { background: #2a0f0f; color: #f87171; border: 1px solid #5a1f1f; }
  .log-time { font-size: 10px; color: var(--muted); margin-left: auto; }
  .log-action { color: var(--text); }
  .log-detail { font-size: 10.5px; color: var(--muted); line-height: 1.4; }
  .log-detail.err { color: var(--fail); }
  .log-empty {
    padding: 16px;
    color: var(--muted);
    font-size: 11px;
    text-align: center;
    font-style: italic;
  }
  .output-row {
    padding: 8px 10px;
    border-top: 1px solid var(--border);
    font-size: 11px;
    font-family: var(--mono);
    color: var(--muted);
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .output-row-label {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--muted);
  }
  .output-path-value {
    color: var(--muted-hi);
    word-break: break-all;
    line-height: 1.4;
  }
</style>
