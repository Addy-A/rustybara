<script>
  import { useAppState } from '../lib/context.js'
  const app = useAppState()

  let inputEl = $state(null)

  $effect(() => {
    if (app.cmdBarVisible && inputEl) {
      requestAnimationFrame(() => inputEl?.focus())
    }
  })

  function parseCmd(input) {
    const t = input.trim()
    if (!t) return null
    const lo = t.toLowerCase()

    if (lo === 'q' || lo === 'quit' || lo === 'exit') return { cmd: 'exit' }
    if (lo === 'theme') return { cmd: 'theme' }
    if (lo === '/n') return { cmd: '/n' }
    if (lo === '/s') return { cmd: '/s' }
    if (lo === 'nq') return { cmd: 'nq' }
    if (lo === 'sa') return { cmd: 'sa' }
    if (lo === 'sd') return { cmd: 'sd' }

    // s (scope current+next) or sN (scope only file N)
    const sMatch = t.match(/^s(\d+)?$/i)
    if (sMatch) {
      const n = sMatch[1]
      return { cmd: 's', index: n != null ? parseInt(n, 10) - 1 : null }
    }

    // v (viewer all scoped) or vN (viewer file N)
    const vMatch = t.match(/^v(\d+)?$/i)
    if (vMatch) {
      const n = vMatch[1]
      return { cmd: 'v', index: n != null ? parseInt(n, 10) - 1 : null }
    }

    // delete buffers
    const m = t.match(/^([0-9,\-\s]*)?(bd|ba)$/i)
    if (!m) return null
    const rangeStr = (m[1] ?? '').trim()
    const cmd = m[2].toLowerCase()
    if (cmd === 'ba') return { cmd: 'ba', indices: null }
    if (!rangeStr) return { cmd: 'bd', indices: [0] }
    const indices = []
    for (const part of rangeStr.split(',')) {
      const p = part.trim()
      const rm = p.match(/^(\d+)-(\d+)$/)
      if (rm) {
        const from = parseInt(rm[1]) - 1
        const to = parseInt(rm[2]) - 1
        for (let i = from; i <= to; i++) if (i >= 0) indices.push(i)
      } else {
        const n = parseInt(p)
        if (!isNaN(n) && n >= 1) indices.push(n - 1)
      }
    }
    return { cmd: 'bd', indices: [...new Set(indices)].sort((a, b) => a - b) }
  }

  let parsed = $derived(parseCmd(app.cmdBarInput))

  let previewFiles = $derived.by(() => {
    if (!parsed) return null
    if (parsed.cmd === 'ba') return app.files
    if (parsed.cmd === 'bd') return parsed.indices.map((i) => app.files[i]).filter(Boolean)
    if (parsed.cmd === 'v') {
      if (parsed.index != null) return [app.files[parsed.index]].filter(Boolean)
      return app.files.filter((f) => f.scoped)
    }
    if (parsed.cmd === 's') {
      if (parsed.index != null) return [app.files[parsed.index]].filter(Boolean)
      const base = app.activeFile ?? 0
      return [app.files[base], app.files[base + 1]].filter(Boolean)
    }
    return null
  })

  let isValid = $derived.by(() => {
    if (!parsed) return false
    switch (parsed.cmd) {
      case 'exit':
      case 'theme':
      case '/n':
      case '/s':
      case 'nq':
        return true
      case 'sa':
      case 'sd':
      case 's':
        return app.files.length > 0
      case 'v':
        if (parsed.index != null) return parsed.index < app.files.length
        return app.files.filter((f) => f.scoped).length > 0
      case 'ba':
        return app.files.length > 0
      case 'bd':
        return (previewFiles?.length ?? 0) > 0
      default:
        return false
    }
  })

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      app.closeCmdBar()
      e.preventDefault()
      e.stopPropagation()
    } else if (e.key === 'Enter') {
      if (isValid) app.executeCmdBar(parsed)
      e.preventDefault()
    }
  }
</script>

{#if app.cmdBarVisible}
  <div class="cmdbar">
    {#if app.cmdBarInput.length > 0}
      <div class="cmdbar-preview" class:valid={isValid} class:invalid={!isValid}>
        {#if !parsed}
          <span class="text dim">unknown command</span>
        {:else if parsed.cmd === 'nq'}
          <span class="text">load a fresh set of random quips</span>
        {:else if parsed.cmd === 'exit'}
          <span class="text">close rbara</span>
        {:else if parsed.cmd === 'theme'}
          <span class="text">toggle theme → {app.theme === 'dark' ? 'light' : 'dark'}</span>
        {:else if parsed.cmd === '/n'}
          <span class="text">pick custom output folder…</span>
        {:else if parsed.cmd === '/s'}
          <span class="text">set output → same folder as source</span>
        {:else if parsed.cmd === 'sa'}
          <span class="text"
            >scope all {app.files.length} buffer{app.files.length !== 1 ? 's' : ''}</span
          >
        {:else if parsed.cmd === 'sd'}
          <span class="text">scope out all buffers</span>
        {:else if parsed.cmd === 's'}
          {#if !isValid}
            <span class="text dim">no files loaded</span>
          {:else if parsed.index != null}
            <span class="label">scope only:</span>
            {#each previewFiles as f (f.path)}<span class="file">{f.name}</span>{/each}
          {:else}
            <span class="label">scope:</span>
            {#each previewFiles as f, i (f.path)}
              {#if i > 0}<span class="sep">,</span>{/if}
              <span class="file">{f.name}</span>
            {/each}
          {/if}
        {:else if parsed.cmd === 'v'}
          {#if !isValid}
            <span class="text dim"
              >{parsed.index != null ? 'file not found' : 'no scoped files'}</span
            >
          {:else}
            <span class="label">open viewer:</span>
            {#each previewFiles as f, i (f.path)}
              {#if i > 0}<span class="sep">,</span>{/if}
              <span class="file">{f.name}</span>
            {/each}
          {/if}
        {:else if parsed.cmd === 'ba'}
          <span class="text"
            >delete all {app.files.length} buffer{app.files.length !== 1 ? 's' : ''}</span
          >
        {:else if previewFiles.length === 0}
          <span class="text dim">no matching buffers</span>
        {:else}
          <span class="label">delete:</span>
          {#each previewFiles as f, i (f.path)}
            {#if i > 0}<span class="sep">,</span>{/if}
            <span class="file">{f.name}</span>
          {/each}
        {/if}
      </div>
    {/if}
    <div class="input-row">
      <span class="colon">:</span>
      <input
        bind:this={inputEl}
        type="text"
        bind:value={app.cmdBarInput}
        onkeydown={handleKeydown}
        spellcheck="false"
        autocomplete="off"
      />
    </div>
  </div>
{/if}

<style>
  .cmdbar {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    z-index: 500;
    background: var(--surface);
    border-top: 2px solid var(--orange);
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .cmdbar-preview {
    padding: 5px 12px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px;
    min-height: 26px;
  }
  .cmdbar-preview.valid {
    background: var(--orange-dim);
  }
  .cmdbar-preview.invalid {
    background: var(--panel);
  }
  .label {
    color: var(--muted);
  }
  .text {
    color: var(--text);
  }
  .text.dim {
    color: var(--muted);
    font-style: italic;
  }
  .file {
    color: var(--orange-hi);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 0 4px;
  }
  .sep {
    color: var(--muted);
  }
  .input-row {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 4px 8px;
  }
  .colon {
    color: var(--orange-hi);
    font-size: 13px;
    user-select: none;
  }
  input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text);
    font-family: var(--mono);
    font-size: 12.5px;
    caret-color: var(--orange);
    padding: 0;
  }
</style>
