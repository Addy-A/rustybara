<script>
  import { useAppState } from '../lib/context.js';
  import Notice from './Notice.svelte';
  import RunButton from './RunButton.svelte';
  const app = useAppState();
</script>

<div class="header">
  <span class="title-icon">✦</span>
  <div>
    <div class="params-title">Flatten Spot Colors</div>
    <div class="params-desc">
      Replaces <code>Separation</code> spot ink operators with device CMYK equivalents,
      evaluated from each spot color's embedded tint function.
      No ICC profile is applied — this is a pure spot-to-CMYK substitution.
    </div>
  </div>
</div>

<div class="info-box">
  <div class="info-row">
    <span class="info-label">Input</span>
    <span class="info-val">Separation color space <code>cs</code> / <code>scn</code> operators</span>
  </div>
  <div class="info-row">
    <span class="info-label">Output</span>
    <span class="info-val">Device CMYK <code>k</code> / <code>K</code> operators</span>
  </div>
  <div class="info-row">
    <span class="info-label">Note</span>
    <span class="info-val">DeviceN multi-channel inks are detected but not flattened.</span>
  </div>
</div>

{#if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else if app.metadata.color_space === 'PureRGB'}
  <Notice ok={false}>Pure RGB document — no spot colors expected.</Notice>
{:else}
  <Notice ok>Ready to flatten spot colors.</Notice>
{/if}

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

<RunButton label="Flatten Spots" icon="✦" />

<style>
  .header { display: flex; align-items: flex-start; gap: 10px; }
  .title-icon { font-size: 20px; color: var(--orange); flex-shrink: 0; padding-top: 1px; }
  .params-title { font-size: 13px; font-weight: 700; color: var(--text); }
  .params-desc {
    font-size: 11.5px;
    color: var(--muted-hi);
    line-height: 1.55;
    margin-top: 2px;
  }
  .params-desc code {
    font-family: var(--mono);
    font-size: 10.5px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 0 3px;
  }
  .info-box {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .info-row { display: flex; gap: 10px; font-size: 11.5px; }
  .info-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--muted);
    width: 46px;
    flex-shrink: 0;
    padding-top: 1px;
  }
  .info-val { color: var(--muted-hi); line-height: 1.45; }
  .info-val code {
    font-family: var(--mono);
    font-size: 10.5px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 0 3px;
  }
  .hint { font-family: var(--mono); font-size: 11px; color: var(--muted); }
</style>
