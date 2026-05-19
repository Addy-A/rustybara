<script>
  let { spots = [], onClose } = $props()
</script>

<div class="overlay" onclick={onClose} role="presentation">
  <div
    class="modal"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => { if (e.key === 'Escape') onClose() }}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="modal-header">
      <span class="title">Spot Colors</span>
      <button class="close-btn" onclick={onClose} aria-label="Close">×</button>
    </div>

    {#if spots.length === 0}
      <div class="empty">No spot color inks detected.</div>
    {:else}
      <ul class="spot-list">
        {#each spots as name}
          <li class="spot-row">
            <span class="spot-icon">✦</span>
            <span class="spot-name">{name}</span>
          </li>
        {/each}
      </ul>
    {/if}
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
    width: 320px;
    max-width: 94vw;
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 10px 40px #000a;
    overflow: hidden;
  }
  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px 11px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .title {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text);
  }
  .close-btn {
    background: none;
    border: none;
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    color: var(--muted);
    padding: 0 2px;
    transition: color 0.1s;
  }
  .close-btn:hover {
    color: var(--text);
  }
  .spot-list {
    list-style: none;
    margin: 0;
    padding: 8px 0;
    overflow-y: auto;
  }
  .spot-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 16px;
  }
  .spot-row:hover {
    background: var(--panel);
  }
  .spot-icon {
    color: var(--orange);
    font-size: 10px;
    flex-shrink: 0;
  }
  .spot-name {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text);
  }
  .empty {
    padding: 20px 16px;
    font-size: 12px;
    color: var(--muted);
    font-family: var(--mono);
  }
</style>
