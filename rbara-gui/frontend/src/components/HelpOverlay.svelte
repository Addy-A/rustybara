<script>
  import { useAppState } from '../lib/context.js';
  const app = useAppState();

  const shortcuts = [
    ['t', 'Trim Marks'],
    ['r', 'Resize to Bleed'],
    ['x', 'Export Images'],
    ['m', 'Remap Colors'],
    ['/', 'Output Path'],
    ['o', 'Toggle overwrite'],
    ['f', 'Add files…'],
    ['a', 'Scope all files'],
    ['n', 'Scope no files'],
    ['i', 'Invert file scope'],
    ['Enter', 'Run active action'],
    ['?', 'Toggle this help'],
    ['Esc', 'Close help'],
  ];
</script>

<div class="overlay" onclick={() => (app.helpVisible = false)} role="presentation">
  <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog">
    <div class="title">Keyboard Shortcuts</div>
    <div class="grid">
      {#each shortcuts as [k, label]}
        <div class="key">{k}</div>
        <div class="label">{label}</div>
      {/each}
    </div>
    <button class="close" onclick={() => (app.helpVisible = false)}>Close</button>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: #000a;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  .modal {
    background: var(--surface);
    border: 1px solid var(--border-hi);
    border-radius: 8px;
    padding: 20px 24px;
    min-width: 320px;
    box-shadow: 0 10px 40px #000a;
  }
  .title {
    font-size: 14px;
    font-weight: 700;
    color: var(--orange);
    margin-bottom: 14px;
  }
  .grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 8px 18px;
    margin-bottom: 16px;
  }
  .key {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    padding: 2px 8px;
    border-radius: 3px;
    text-align: center;
  }
  .label { font-size: 12px; color: var(--muted-hi); }
  .close {
    background: var(--orange);
    color: #fff;
    border: none;
    border-radius: 5px;
    padding: 6px 16px;
    font-weight: 700;
    font-size: 12px;
  }
  .close:hover { background: var(--orange-hi); }
</style>
