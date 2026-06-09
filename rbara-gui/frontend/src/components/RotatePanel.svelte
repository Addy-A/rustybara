<script>
  import { useAppState } from '../lib/context.js'
  import Notice from './Notice.svelte'
  import RunButton from './RunButton.svelte'
  const app = useAppState()

  const angles = [90, 180, 270]
</script>

<div class="header">
  <span class="title-icon">⟳</span>
  <div>
    <div class="params-title">Rotate PDF</div>
    <div class="params-desc">
      Rotates every page's display orientation clockwise. Applied on top of any
      existing rotation.
    </div>
  </div>
</div>

<div class="param-group">
  <div class="param-label">Rotation</div>
  <div class="angle-grid">
    {#each angles as a}
      <button
        class="angle-btn"
        class:sel={app.params.rotateDegrees === a}
        onclick={() => (app.params.rotateDegrees = a)}>{a}°</button
      >
    {/each}
  </div>
</div>

{#if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else}
  <Notice ok
    >Rotates {app.metadata.page_count} page(s) by {app.params.rotateDegrees}° clockwise.</Notice
  >
{/if}

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

<RunButton label="Run Rotate" icon="⟳" />

<style>
  .header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .title-icon {
    font-size: 20px;
    color: var(--orange);
  }
  .params-title {
    font-size: 13px;
    font-weight: 700;
    color: var(--text);
  }
  .params-desc {
    font-size: 11.5px;
    color: var(--muted-hi);
    line-height: 1.5;
    margin-top: 2px;
  }
  .param-group {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .param-label {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--muted-hi);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .angle-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
    max-width: 240px;
  }
  .angle-btn {
    padding: 6px 0;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    font-size: 12px;
    color: var(--muted-hi);
    text-align: center;
    font-family: var(--mono);
  }
  .angle-btn.sel {
    background: var(--orange-dim);
    border-color: var(--orange);
    color: var(--orange-hi);
  }
  .angle-btn:hover {
    border-color: var(--border-hi);
    color: var(--text);
  }
  .hint {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--muted);
  }
</style>
