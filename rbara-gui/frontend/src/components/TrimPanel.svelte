<script>
  import { useAppState } from '../lib/context.js';
  import Notice from './Notice.svelte';
  import RunButton from './RunButton.svelte';
  const app = useAppState();
</script>

<div class="header">
  <span class="title-icon">✂</span>
  <div>
    <div class="params-title">Trim Marks</div>
    <div class="params-desc">Removes content outside the TrimBox of every page.</div>
  </div>
</div>

{#if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else if !app.canTrim}
  <Notice ok={false}>No TrimBox detected — this file cannot be trimmed.</Notice>
{:else}
  <Notice ok>TrimBox detected — ready to trim. No issues found.</Notice>
{/if}

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

<RunButton label="Run Trim Marks" icon="✂" disabled={!app.canTrim} />

<style>
  .header { display: flex; align-items: center; gap: 10px; }
  .title-icon { font-size: 20px; color: var(--orange); }
  .params-title { font-size: 13px; font-weight: 700; color: var(--text); }
  .params-desc { font-size: 11.5px; color: var(--muted-hi); line-height: 1.5; margin-top: 2px; }
  .hint { font-family: var(--mono); font-size: 11px; color: var(--muted); }
</style>
