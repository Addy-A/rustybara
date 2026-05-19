<script>
  import { useAppState } from '../lib/context.js'
  import { colorSpaceTagClass, colorSpaceLabel, formatSize } from '../lib/api.js'
  const app = useAppState()
</script>

<div class="drop-zone">
  <span class="drop-label">
    Files
    {#if app.files.length > 0}
      <span class="scope-count">({app.scopedCount}/{app.files.length})</span>
    {/if}
    :
  </span>

  {#if app.files.length > 0}
    <div class="bulk">
      <button
        class="bulk-btn"
        onclick={() => app.scopeAll()}
        title="Scope all (a)">All</button
      >
      <button
        class="bulk-btn"
        onclick={() => app.scopeNone()}
        title="Scope none (n)">None</button
      >
      <button
        class="bulk-btn"
        onclick={() => app.invertScope()}
        title="Invert scope (i)">Inv</button
      >
    </div>
  {/if}

  {#each app.files as f, i (f.path)}
    <div
      class="file-chip"
      class:active={app.activeFile === i}
      class:unscoped={!f.scoped}
      onclick={() => app.selectFile(i)}
      role="button"
      tabindex="0"
    >
      <span class="chip-idx">{i + 1}</span>
      <span
        class="chip-check"
        class:on={f.scoped}
        onclick={(e) => {
          e.stopPropagation()
          app.toggleScope(i)
        }}
        role="checkbox"
        aria-checked={f.scoped}
        tabindex="0"
        title={f.scoped
          ? 'Scoped — click to exclude'
          : 'Excluded — click to scope'}>{f.scoped ? '✓' : ''}</span
      >
      <span class="chip-name">{f.name}</span>
      <span class="chip-cs {colorSpaceTagClass(f.colorSpace)}"
        >{colorSpaceLabel(f.colorSpace)}</span
      >
      <span class="chip-size">{formatSize(f.sizeKb)}</span>
      <span
        class="chip-x"
        onclick={(e) => {
          e.stopPropagation()
          app.removeFile(i)
        }}
        role="button"
        tabindex="0">×</span
      >
    </div>
  {/each}
  <button
    class="add-file-btn"
    onclick={() => app.addFiles()}
    disabled={app.processing}
  >
    ＋ Add
  </button>
  <div class="overwrite-toggle">
    <div
      class="toggle-pill"
      class:on={app.overwrite}
      onclick={() => (app.overwrite = !app.overwrite)}
      role="switch"
      aria-checked={app.overwrite}
      tabindex="0"
    ></div>
    Overwrite<span class="hk">o</span>
  </div>
</div>

<style>
  .drop-zone {
    min-height: 38px;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    padding: 6px 14px;
    gap: 8px;
    flex-shrink: 0;
    flex-wrap: wrap;
  }
  .drop-label {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
  }
  .scope-count {
    color: var(--orange);
    font-family: var(--mono);
    margin-left: 2px;
  }
  .bulk {
    display: flex;
    gap: 2px;
    border: 1px solid var(--border);
    border-radius: 4px;
    overflow: hidden;
    background: var(--panel);
  }
  .bulk-btn {
    background: transparent;
    border: none;
    color: var(--muted-hi);
    font-size: 10px;
    padding: 3px 7px;
    font-family: var(--sans);
  }
  .bulk-btn:hover {
    background: var(--orange-dim);
    color: var(--orange-hi);
  }
  .file-chip {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 4px;
    font-size: 11px;
    font-family: var(--mono);
    color: var(--text);
    cursor: pointer;
    transition: opacity 0.12s;
  }
  .file-chip:hover {
    border-color: var(--border-hi);
  }
  .file-chip.active {
    border-color: var(--orange);
    color: var(--orange-hi);
  }
  .file-chip.unscoped {
    opacity: 0.45;
    background: transparent;
  }
  .chip-idx {
    font-size: 9px;
    color: var(--muted);
    font-family: var(--mono);
    min-width: 10px;
    text-align: right;
    flex-shrink: 0;
    user-select: none;
  }
  .chip-check {    width: 13px;
    height: 13px;
    border-radius: 3px;
    border: 1px solid var(--border-hi);
    background: var(--bg);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    line-height: 1;
    color: transparent;
    flex-shrink: 0;
  }
  .chip-check.on {
    background: var(--orange);
    border-color: var(--orange);
    color: #fff;
  }
  .chip-check:hover {
    border-color: var(--orange);
  }
  .chip-name {
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chip-cs {
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 3px;
  }
  .chip-cs.cmyk {
    background: #2a1500;
    color: #f97316;
    border: 1px solid #4a2500;
  }
  .chip-cs.rgb {
    background: #0f1e30;
    color: #60a5fa;
    border: 1px solid #1e3a5f;
  }
  .chip-cs.mixed {
    background: #1a1a00;
    color: #fbbf24;
    border: 1px solid #3a3a00;
  }
  .chip-size {
    font-size: 10px;
    color: var(--muted);
  }
  .chip-x {
    color: var(--muted);
    cursor: pointer;
    font-size: 13px;
    line-height: 1;
  }
  .chip-x:hover {
    color: var(--fail);
  }
  .add-file-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    background: none;
    border: 1px dashed var(--border-hi);
    border-radius: 4px;
    font-size: 11px;
    color: var(--muted);
    font-family: var(--sans);
  }
  .add-file-btn:hover:not(:disabled) {
    border-color: var(--orange);
    color: var(--orange);
  }
  .overwrite-toggle {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--muted);
  }
  .toggle-pill {
    width: 28px;
    height: 15px;
    background: var(--border);
    border-radius: 8px;
    position: relative;
    cursor: pointer;
    flex-shrink: 0;
  }
  .toggle-pill::after {
    content: '';
    position: absolute;
    width: 11px;
    height: 11px;
    background: #fff;
    border-radius: 50%;
    top: 2px;
    left: 2px;
    transition: left 0.15s;
  }
  .toggle-pill.on {
    background: var(--orange);
  }
  .toggle-pill.on::after {
    left: 15px;
  }
</style>
