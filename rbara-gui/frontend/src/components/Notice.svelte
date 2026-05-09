<script>
  let { ok = true, children } = $props();
</script>

<div class="notice" class:ok class:warn={!ok}>
  <span class="icon">{ok ? '✓' : '⚠'}</span>
  <span class="text">{@render children?.()}</span>
</div>

<style>
  .notice {
    border-radius: 6px;
    padding: 10px 12px;
    font-size: 11.5px;
    display: flex;
    gap: 8px;
    align-items: flex-start;
    background: var(--panel);
    border: 1px solid var(--border);
  }
  .notice.ok { border-color: #065f46; color: var(--ok); }
  .notice.warn { border-color: #78400080; color: var(--warn); }
  .icon { font-size: 15px; flex-shrink: 0; }
  .text { line-height: 1.4; }
</style>
