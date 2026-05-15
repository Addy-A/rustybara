<script>
  import { useAppState } from '../lib/context.js';
  import Notice from './Notice.svelte';
  import RunButton from './RunButton.svelte';
  const app = useAppState();

  const cmykProfiles = [
    { value: 'CoatedFOGRA27',           label: 'Coated FOGRA 27' },
    { value: 'CoatedFOGRA39',           label: 'Coated FOGRA 39' },
    { value: 'CoatedGRACoL2006',        label: 'Coated GRACoL 2006' },
    { value: 'JapanColor2001Coated',    label: 'Japan Color 2001 Coated' },
    { value: 'JapanColor2001Uncoated',  label: 'Japan Color 2001 Uncoated' },
    { value: 'JapanColor2002Newspaper', label: 'Japan Color 2002 Newspaper' },
    { value: 'JapanColor2003WebCoated', label: 'Japan Color 2003 Web Coated' },
    { value: 'JapanWebCoated',          label: 'Japan Web Coated' },
    { value: 'UncoatedFOGRA29',         label: 'Uncoated FOGRA 29' },
    { value: 'USWebCoatedSWOP',         label: 'US Web Coated SWOP' },
    { value: 'USWebUncoated',           label: 'US Web Uncoated' },
    { value: 'WebCoatedFOGRA28',        label: 'Web Coated FOGRA 28' },
    { value: 'WebCoatedSWOP2006Grade3', label: 'Web Coated SWOP 2006 Grade 3' },
    { value: 'WebCoatedSWOP2006Grade5', label: 'Web Coated SWOP 2006 Grade 5' },
  ];

  const rgbProfiles = [
    { value: 'AdobeRGB1998',  label: 'Adobe RGB (1998)' },
    { value: 'AppleRGB',      label: 'Apple RGB' },
    { value: 'ColorMatchRGB', label: 'ColorMatch RGB' },
    { value: 'PAL_SECAM',     label: 'PAL/SECAM' },
    { value: 'SMPTE-C',       label: 'SMPTE-C' },
    { value: 'VideoHD',       label: 'HDTV (Rec. 709)' },
    { value: 'VideoNTSC',     label: 'NTSC (1953)' },
    { value: 'VideoPAL',      label: 'PAL (Video)' },
  ];

  const intents = [
    {
      value: 'RelativeColorimetric',
      label: 'Relative Colorimetric',
      desc: 'Best for print. In-gamut colors stay exact; out-of-gamut colors clip to the nearest match. White point adapts to the destination.',
    },
    {
      value: 'Perceptual',
      label: 'Perceptual',
      desc: 'Scales the whole gamut to fit, preserving visual relationships. Colors may shift slightly but nothing clips. Good for photos.',
    },
    {
      value: 'Saturation',
      label: 'Saturation',
      desc: 'Keeps colors vibrant at the expense of accuracy. Suited for charts, logos, and graphics where punchy color matters most.',
    },
    {
      value: 'AbsoluteColorimetric',
      label: 'Absolute Colorimetric',
      desc: 'Like Relative but does not adjust the white point. Used for soft-proofing to simulate one substrate on another.',
    },
  ];

  let sameProfile = $derived(app.params.fromProfile === app.params.toProfile);
</script>

<div class="header">
  <span class="title-icon">◈</span>
  <div>
    <div class="params-title">Convert Color Space</div>
    <div class="params-desc">Applies an ICC transform to every CMYK/RGB paint operator in the document's content streams.</div>
  </div>
</div>

<div class="param-group">
  <div class="param-label">Source Profile</div>
  <select class="param-select" bind:value={app.params.fromProfile}>
    <optgroup label="CMYK">
      {#each cmykProfiles as p (p.value)}
        <option value={p.value}>{p.label}</option>
      {/each}
    </optgroup>
    <optgroup label="RGB">
      {#each rgbProfiles as p (p.value)}
        <option value={p.value}>{p.label}</option>
      {/each}
    </optgroup>
  </select>
</div>

<div class="arrow-row">
  <span class="arrow">↓</span>
</div>

<div class="param-group">
  <div class="param-label">Destination Profile</div>
  <select class="param-select" bind:value={app.params.toProfile}>
    <optgroup label="CMYK">
      {#each cmykProfiles as p (p.value)}
        <option value={p.value}>{p.label}</option>
      {/each}
    </optgroup>
    <optgroup label="RGB">
      {#each rgbProfiles as p (p.value)}
        <option value={p.value}>{p.label}</option>
      {/each}
    </optgroup>
  </select>
</div>

<div class="param-group">
  <div class="param-label">Rendering Intent</div>
  <div class="intent-list">
    {#each intents as i (i.value)}
      <button
        class="intent-card"
        class:sel={app.params.convertIntent === i.value}
        onclick={() => (app.params.convertIntent = i.value)}
      >
        <div class="intent-dot"></div>
        <div class="intent-text">
          <div class="intent-name">{i.label}</div>
          <div class="intent-desc">{i.desc}</div>
        </div>
      </button>
    {/each}
  </div>
</div>

{#if sameProfile}
  <Notice ok={false}>Source and destination profiles are the same — no conversion will occur.</Notice>
{:else if !app.metadata}
  <Notice ok={false}>Load a file to validate.</Notice>
{:else}
  <Notice ok>Ready to convert.</Notice>
{/if}

{#if app.outputHint}
  <div class="hint">{app.outputHint}</div>
{/if}

<RunButton label="Run Conversion" icon="◈" />

<style>
  .header { display: flex; align-items: center; gap: 10px; }
  .title-icon { font-size: 20px; color: var(--orange); flex-shrink: 0; }
  .params-title { font-size: 13px; font-weight: 700; color: var(--text); }
  .params-desc { font-size: 11.5px; color: var(--muted-hi); line-height: 1.5; margin-top: 2px; }
  .param-group { display: flex; flex-direction: column; gap: 7px; }
  .param-label {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--muted-hi);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .param-select {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 7px 10px;
    color: var(--text);
    font-size: 12px;
    outline: none;
    width: 100%;
  }
  .param-select:focus { border-color: var(--orange); }
  .arrow-row { display: flex; justify-content: center; padding: 2px 0; }
  .arrow { font-size: 20px; color: var(--orange); }
  .hint { font-family: var(--mono); font-size: 11px; color: var(--muted); }

  .intent-list { display: flex; flex-direction: column; gap: 5px; }
  .intent-card {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 8px 10px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    text-align: left;
    cursor: pointer;
    transition: 0.1s;
    width: 100%;
  }
  .intent-card:hover { border-color: var(--muted-hi); }
  .intent-card.sel {
    border-color: var(--orange);
    background: var(--orange-dim);
  }
  .intent-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--border);
    background: transparent;
    flex-shrink: 0;
    margin-top: 3px;
    transition: 0.1s;
  }
  .intent-card.sel .intent-dot {
    border-color: var(--orange);
    background: var(--orange);
  }
  .intent-text { display: flex; flex-direction: column; gap: 2px; }
  .intent-name { font-size: 12px; font-weight: 600; color: var(--text); }
  .intent-card.sel .intent-name { color: var(--orange-hi); }
  .intent-desc { font-size: 11px; color: var(--muted-hi); line-height: 1.5; }
</style>
