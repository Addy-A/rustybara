<script>
  let { message, onTurnOff, onKeep } = $props()
</script>

<div class="for-banner" role="alert">
  <span class="for-icon">⚠</span>
  <span class="for-msg">{message}</span>
  <div class="for-actions">
    <button class="for-btn turn-off" onclick={onTurnOff}>Turn off overwrite</button>
    <button class="for-btn keep" onclick={onKeep}>Keep on</button>
  </div>
</div>

<style>
  .for-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 14px;
    background: color-mix(in srgb, var(--warn) 10%, var(--surface));
    border-bottom: 1px solid color-mix(in srgb, var(--warn) 35%, transparent);
    flex-shrink: 0;
    font-size: 11.5px;
  }
  .for-icon { color: var(--warn); flex-shrink: 0; }
  .for-msg  { color: var(--text); flex: 1; line-height: 1.4; }
  .for-actions { display: flex; gap: 6px; flex-shrink: 0; }
  .for-btn {
    padding: 4px 10px;
    border-radius: 4px;
    font-size: 11px;
    cursor: pointer;
    border: 1px solid var(--border);
    background: var(--panel);
    color: var(--muted-hi);
    white-space: nowrap;
  }
  .for-btn:hover { border-color: var(--border-hi); color: var(--text); }
  .for-btn.turn-off {
    border-color: color-mix(in srgb, var(--warn) 40%, transparent);
    color: var(--warn);
  }
  .for-btn.turn-off:hover { background: color-mix(in srgb, var(--warn) 12%, var(--panel)); }
</style>