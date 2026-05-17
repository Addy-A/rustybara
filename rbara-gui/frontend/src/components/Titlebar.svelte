<script>
  import { useAppState } from '../lib/context.js'
  const app = useAppState()
</script>

<div class="titlebar">
  <span class="app-name">rustybara</span>
  <span class="app-sub">— Prepress Toolkit</span>
  <span class="version-badge">v0.1.4-alpha</span>
</div>

<style>
  .titlebar {
    height: 36px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    padding: 0 14px;
    gap: 10px;
    flex-shrink: 0;
    user-select: none;
  }
  .traffic {
    display: flex;
    gap: 6px;
  }
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
  }
  .app-name {
    font-size: 12px;
    font-weight: 700;
    color: var(--orange);
    letter-spacing: 0.04em;
  }
  .app-sub {
    font-size: 11px;
    color: var(--muted);
    letter-spacing: 0.04em;
  }
  .version-badge {
    margin-left: auto;
    font-size: 10px;
    color: var(--muted);
    background: var(--panel);
    border: 1px solid var(--border);
    padding: 2px 8px;
    border-radius: 10px;
    font-family: var(--mono);
  }
</style>
