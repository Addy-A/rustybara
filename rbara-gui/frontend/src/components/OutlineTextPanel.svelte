<script>
  import { useAppState } from '../lib/context.js';
  import Notice from './Notice.svelte';
  import RunButton from './RunButton.svelte';
  const app = useAppState();
</script>

<div class="header">
  <span class="title-icon">⊤</span>
  <div>
    <div class="params-title">Outline Text</div>
    <div class="params-desc">
      Converts all text on every page to vector paths. Fonts are no longer
      required for rendering.
    </div>
  </div>
</div>

{#if !app.metadata}
  <Notice ok={false}>Load a file to use Outline Text.</Notice>
{:else}
  <Notice ok>
    Ready to outline. All text will be converted to filled paths in CMYK black.
    Note: colored text color preservation is not yet implemented.
  </Notice>
{/if}

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

<RunButton label="Run Outline Text" icon="⊤" disabled={!app.metadata} />

<style>
  .header { display: flex; align-items: center; gap: 10px; }
  .title-icon { font-size: 20px; color: var(--orange); }
  .params-title { font-size: 13px; font-weight: 700; color: var(--text); }
  .params-desc { font-size: 11.5px; color: var(--muted-hi); line-height: 1.5; margin-top: 2px; }
  .hint { font-family: var(--mono); font-size: 11px; color: var(--muted); }
</style>