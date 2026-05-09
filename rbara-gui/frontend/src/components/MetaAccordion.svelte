<script>
  import { useAppState } from '../lib/context.js';
  import MetaGrid from './MetaGrid.svelte';
  const app = useAppState();

  let open = $state(false);
</script>

{#if app.metadata}
  <div class="accordion">
    <button class="acc-header" onclick={() => (open = !open)}>
      <span>Metadata</span>
      <span class="acc-arrow" class:open>▸</span>
    </button>
    {#if open}
      <div class="acc-body">
        <MetaGrid compact />
      </div>
    {/if}
  </div>
{/if}

<style>
  .accordion {
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    flex-shrink: 0;
  }
  .acc-header {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 14px;
    background: transparent;
    border: none;
    color: var(--muted-hi);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  .acc-arrow {
    transition: transform 0.15s;
    color: var(--orange);
  }
  .acc-arrow.open { transform: rotate(90deg); }
  .acc-body { border-top: 1px solid var(--border); }
</style>
