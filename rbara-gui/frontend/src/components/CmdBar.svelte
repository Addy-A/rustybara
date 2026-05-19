<script>
  import { useAppState } from '../lib/context.js'
  import {
    BUILTIN_PROFILES,
    listDirs,
    listPdfFiles,
    basename,
  } from '../lib/api.js'
  const app = useAppState()

  let inputEl = $state(null)
  let optionListEl = $state(null)
  let selectedOptionIdx = $state(0)

  $effect(() => {
    if (app.cmdBarVisible && inputEl) {
      requestAnimationFrame(() => inputEl?.focus())
    }
  })

  // All profiles: builtin + custom
  let allProfiles = $derived([
    ...BUILTIN_PROFILES,
    ...app.customProfiles.map((p) => ({
      value: p.name,
      label: p.description,
      color_space: p.color_space,
    })),
  ])

  // Active file's directory — base for /n:: and f:: searches
  let activeFileDir = $derived.by(() => {
    const f = app.files[app.activeFile ?? 0]
    if (!f) return null
    const p = f.path
    const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'))
    return i >= 0 ? p.slice(0, i) : p
  })

  function joinPath(base, rel) {
    if (!rel) return base
    return base.replace(/[\\/]+$/, '') + '\\' + rel.replace(/\//g, '\\')
  }

  function getDirQueryParts(query) {
    const normalized = query.replace(/\\/g, '/')
    const lastSlash = normalized.lastIndexOf('/')
    if (lastSlash === -1) return { relParent: '', filter: query }
    return {
      relParent: query.slice(0, lastSlash),
      filter: query.slice(lastSlash + 1),
    }
  }

  function parseCmd(input) {
    const t = input.trim()
    if (!t) return null
    const lo = t.toLowerCase()

    if (lo === 'q' || lo === 'quit' || lo === 'exit') return { cmd: 'exit' }
    if (lo === 'minimize' || lo === 'min' || lo === 'hide')
      return { cmd: 'minimize' }
    if (lo === 'full' || lo === 'max' || lo === 'maximize')
      return { cmd: 'maximize' }
    if (lo === 'theme') return { cmd: 'theme' }
    if (lo === '/n') return { cmd: '/n' }
    if (lo === '/s') return { cmd: '/s' }
    if (lo === 'nq') return { cmd: 'nq' }
    if (lo === 'sa') return { cmd: 'sa' }
    if (lo === 'sd') return { cmd: 'sd' }

    // csrc:: and cdst:: profile picker
    const csrcMatch = t.match(/^csrc::(.*)$/i)
    if (csrcMatch) return { cmd: 'csrc', query: csrcMatch[1] }
    const cdstMatch = t.match(/^cdst::(.*)$/i)
    if (cdstMatch) return { cmd: 'cdst', query: cdstMatch[1] }

    // /n:: directory picker (output folder)
    const ndirMatch = t.match(/^\/n::(.*)$/i)
    if (ndirMatch) return { cmd: '/n-dir', query: ndirMatch[1] }

    // f:: PDF file picker (add to buffer)
    const ffileMatch = t.match(/^f::(.*)$/i)
    if (ffileMatch) return { cmd: 'f-file', query: ffileMatch[1] }

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

    // delete / scope range commands: N, N-M, N,M,K  prefix
    const m = t.match(/^([0-9,\-\s]*)?(bd|ba|sd|s)$/i)
    if (!m) return null
    const rangeStr = (m[1] ?? '').trim()
    const cmd = m[2].toLowerCase()
    if (cmd === 'ba') return { cmd: 'ba', indices: null }
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
    const sorted = [...new Set(indices)].sort((a, b) => a - b)
    if (cmd === 'bd') return { cmd: 'bd', indices: rangeStr ? sorted : [0] }
    if (cmd === 'sd') return { cmd: 'sd', indices: rangeStr ? sorted : null }
    if (cmd === 's')
      return { cmd: 's', indices: rangeStr ? sorted : null, index: null }
    return null
  }

  let parsed = $derived(parseCmd(app.cmdBarInput))

  // --- Profile picker (csrc / cdst) ---
  let filteredProfiles = $derived.by(() => {
    if (!parsed || (parsed.cmd !== 'csrc' && parsed.cmd !== 'cdst')) return []
    const q = parsed.query.trim().toLowerCase()
    if (!q) return allProfiles
    return allProfiles.filter(
      (p) =>
        p.value.toLowerCase().includes(q) || p.label.toLowerCase().includes(q),
    )
  })

  // --- Directory picker (/n::) ---
  let derivedSearchDir = $derived.by(() => {
    if (!parsed || parsed.cmd !== '/n-dir' || !activeFileDir) return null
    const { relParent } = getDirQueryParts(parsed.query)
    return relParent ? joinPath(activeFileDir, relParent) : activeFileDir
  })

  let dirResults = $state([])
  $effect(() => {
    const sd = derivedSearchDir
    if (!sd) {
      dirResults = []
      return
    }
    let cancelled = false
    listDirs(sd)
      .then((dirs) => {
        if (!cancelled) dirResults = dirs
      })
      .catch(() => {
        if (!cancelled) dirResults = []
      })
    return () => {
      cancelled = true
    }
  })

  let filteredDirs = $derived.by(() => {
    if (!parsed || parsed.cmd !== '/n-dir') return []
    const { filter } = getDirQueryParts(parsed.query)
    const q = filter.toLowerCase()
    if (!q) return dirResults
    return dirResults.filter((d) => d.toLowerCase().includes(q))
  })

  // --- PDF file picker (f::) ---
  let derivedPdfDir = $derived.by(() => {
    if (!parsed || parsed.cmd !== 'f-file' || !activeFileDir) return null
    return activeFileDir
  })

  let pdfResults = $state([])
  $effect(() => {
    const dir = derivedPdfDir
    if (!dir) {
      pdfResults = []
      return
    }
    let cancelled = false
    listPdfFiles(dir)
      .then((files) => {
        if (!cancelled) pdfResults = files
      })
      .catch(() => {
        if (!cancelled) pdfResults = []
      })
    return () => {
      cancelled = true
    }
  })

  let filteredPdfs = $derived.by(() => {
    if (!parsed || parsed.cmd !== 'f-file') return []
    const q = parsed.query.trim().toLowerCase()
    if (!q) return pdfResults
    return pdfResults.filter((p) => basename(p).toLowerCase().includes(q))
  })

  // --- Unified option list ---
  let activeOptions = $derived.by(() => {
    if (!parsed) return []
    if (parsed.cmd === 'csrc' || parsed.cmd === 'cdst') return filteredProfiles
    if (parsed.cmd === '/n-dir') return filteredDirs
    if (parsed.cmd === 'f-file') return filteredPdfs
    return []
  })

  $effect(() => {
    activeOptions
    selectedOptionIdx = 0
  })

  // Scroll selected option into view
  $effect(() => {
    const idx = selectedOptionIdx
    if (optionListEl) {
      optionListEl
        .querySelectorAll('.option-row')
        [idx]?.scrollIntoView({ block: 'nearest' })
    }
  })

  let previewFiles = $derived.by(() => {
    if (!parsed) return null
    if (parsed.cmd === 'ba') return app.files
    if (parsed.cmd === 'bd')
      return parsed.indices.map((i) => app.files[i]).filter(Boolean)
    if (parsed.cmd === 'v') {
      if (parsed.index != null) return [app.files[parsed.index]].filter(Boolean)
      return app.files.filter((f) => f.scoped)
    }
    if (parsed.cmd === 's') {
      if (parsed.indices)
        return parsed.indices.map((i) => app.files[i]).filter(Boolean)
      if (parsed.index != null) return [app.files[parsed.index]].filter(Boolean)
      const base = app.activeFile ?? 0
      return [app.files[base], app.files[base + 1]].filter(Boolean)
    }
    if (parsed.cmd === 'sd' && parsed.indices) {
      return parsed.indices.map((i) => app.files[i]).filter(Boolean)
    }
    return null
  })

  let isValid = $derived.by(() => {
    if (!parsed) return false
    switch (parsed.cmd) {
      case 'exit':
      case 'minimize':
      case 'maximize':
      case 'theme':
      case '/n':
      case '/s':
      case 'nq':
        return true
      case 'sa':
        return app.files.length > 0
      case 'sd':
        if (parsed.indices) return (previewFiles?.length ?? 0) > 0
        return app.files.length > 0
      case 's':
        if (parsed.indices) return (previewFiles?.length ?? 0) > 0
        return app.files.length > 0
      case 'v':
        if (parsed.index != null) return parsed.index < app.files.length
        return app.files.filter((f) => f.scoped).length > 0
      case 'ba':
        return app.files.length > 0
      case 'bd':
        return (previewFiles?.length ?? 0) > 0
      case 'csrc':
      case 'cdst':
        return filteredProfiles.length > 0
      case '/n-dir':
        return !!activeFileDir
      case 'f-file':
        return filteredPdfs.length > 0
      default:
        return false
    }
  })

  const isPickerCmd = $derived(
    parsed?.cmd === 'csrc' ||
      parsed?.cmd === 'cdst' ||
      parsed?.cmd === '/n-dir' ||
      parsed?.cmd === 'f-file',
  )

  function fillFromSelection() {
    if (parsed.cmd === 'csrc' || parsed.cmd === 'cdst') {
      app.cmdBarInput = `${parsed.cmd}::${filteredProfiles[selectedOptionIdx].value}`
    } else if (parsed.cmd === '/n-dir') {
      const { relParent } = getDirQueryParts(parsed.query)
      const dir = filteredDirs[selectedOptionIdx]
      app.cmdBarInput = `/n::${relParent ? relParent + '/' + dir : dir}/`
    } else if (parsed.cmd === 'f-file') {
      app.cmdBarInput = `f::${basename(filteredPdfs[selectedOptionIdx])}`
    }
  }

  function executePickerCmd() {
    if (parsed.cmd === 'csrc' || parsed.cmd === 'cdst') {
      app.executeCmdBar({
        cmd: parsed.cmd,
        profile: filteredProfiles[selectedOptionIdx].value,
      })
    } else if (parsed.cmd === '/n-dir') {
      if (filteredDirs.length > 0) {
        const { relParent } = getDirQueryParts(parsed.query)
        const dir = filteredDirs[selectedOptionIdx]
        const absPath = joinPath(derivedSearchDir, dir)
        app.executeCmdBar({ cmd: '/n-dir', path: absPath })
      } else {
        // leaf dir — use the current search dir itself
        app.executeCmdBar({ cmd: '/n-dir', path: derivedSearchDir })
      }
    } else if (parsed.cmd === 'f-file') {
      app.executeCmdBar({
        cmd: 'f-file',
        path: filteredPdfs[selectedOptionIdx],
      })
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      app.closeCmdBar()
      e.preventDefault()
      e.stopPropagation()
    } else if (isPickerCmd && e.key === 'ArrowDown') {
      selectedOptionIdx = Math.min(
        selectedOptionIdx + 1,
        activeOptions.length - 1,
      )
      e.preventDefault()
    } else if (isPickerCmd && e.key === 'ArrowUp') {
      selectedOptionIdx = Math.max(selectedOptionIdx - 1, 0)
      e.preventDefault()
    } else if (isPickerCmd && e.key === 'Tab') {
      if (activeOptions.length > 0) fillFromSelection()
      e.preventDefault()
    } else if (isPickerCmd && e.key === 'Enter') {
      if (isValid) executePickerCmd()
      e.preventDefault()
      e.stopPropagation()
    } else if (e.key === 'Enter') {
      if (isValid) app.executeCmdBar(parsed)
      e.preventDefault()
      e.stopPropagation()
    }
  }
</script>

{#if app.cmdBarVisible}
  <div class="cmdbar">
    {#if app.cmdBarInput.length > 0}
      <div
        class="cmdbar-preview"
        class:valid={isValid}
        class:invalid={!isValid}
      >
        {#if !parsed}
          <span class="text dim">unknown command</span>
        {:else if parsed.cmd === 'nq'}
          <span class="text">load a fresh set of random quips</span>
        {:else if parsed.cmd === 'minimize'}
          <span class="text">minimize window</span>
        {:else if parsed.cmd === 'maximize'}
          <span class="text">toggle maximize window</span>
        {:else if parsed.cmd === 'exit'}
          <span class="text">close rbara</span>
        {:else if parsed.cmd === 'theme'}
          <span class="text"
            >toggle theme → {app.theme === 'dark' ? 'light' : 'dark'}</span
          >
        {:else if parsed.cmd === '/n'}
          <span class="text">pick custom output folder…</span>
        {:else if parsed.cmd === '/s'}
          <span class="text">set output → same folder as source</span>
        {:else if parsed.cmd === 'sa'}
          <span class="text"
            >scope all {app.files.length} buffer{app.files.length !== 1
              ? 's'
              : ''}</span
          >
        {:else if parsed.cmd === 'sd'}
          {#if parsed.indices}
            {#if !isValid}
              <span class="text dim">no matching buffers</span>
            {:else}
              <span class="label">scope out:</span>
              {#each previewFiles as f, i (f.path)}
                {#if i > 0}<span class="sep">,</span>{/if}
                <span class="file">{f.name}</span>
              {/each}
            {/if}
          {:else}
            <span class="text">scope out all buffers</span>
          {/if}
        {:else if parsed.cmd === 's'}
          {#if !isValid}
            <span class="text dim">no files loaded</span>
          {:else if parsed.indices}
            <span class="label">scope in:</span>
            {#each previewFiles as f, i (f.path)}
              {#if i > 0}<span class="sep">,</span>{/if}
              <span class="file">{f.name}</span>
            {/each}
          {:else if parsed.index != null}
            <span class="label">scope only:</span>
            {#each previewFiles as f (f.path)}<span class="file">{f.name}</span
              >{/each}
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
              >{parsed.index != null
                ? 'file not found'
                : 'no scoped files'}</span
            >
          {:else}
            <span class="label">open viewer:</span>
            {#each previewFiles as f, i (f.path)}
              {#if i > 0}<span class="sep">,</span>{/if}
              <span class="file">{f.name}</span>
            {/each}
          {/if}
        {:else if parsed.cmd === 'csrc' || parsed.cmd === 'cdst'}
          {#if filteredProfiles.length === 0}
            <span class="text dim">no matching profiles</span>
          {:else}
            <span class="label"
              >{parsed.cmd === 'csrc'
                ? 'set source profile:'
                : 'set destination profile:'}</span
            >
            <div class="option-list" bind:this={optionListEl}>
              {#each filteredProfiles as p, i (p.value)}
                <div
                  class="option-row profile-row"
                  class:sel={i === selectedOptionIdx}
                >
                  <span class="profile-cs {p.color_space.toLowerCase()}"
                    >{p.color_space}</span
                  >
                  <span class="profile-label">{p.label}</span>
                  <span class="profile-value">{p.value}</span>
                </div>
              {/each}
            </div>
          {/if}
        {:else if parsed.cmd === '/n-dir'}
          {#if !activeFileDir}
            <span class="text dim">no active file — open a PDF first</span>
          {:else if filteredDirs.length === 0}
            <span class="label">set output to:</span>
            <span class="file">{derivedSearchDir ?? activeFileDir}</span>
            <span class="text dim"
              >(no subdirectories — press Enter to confirm)</span
            >
          {:else}
            <span class="label">output folder:</span>
            <div class="option-list" bind:this={optionListEl}>
              {#each filteredDirs as dir, i (dir)}
                <div
                  class="option-row dir-row"
                  class:sel={i === selectedOptionIdx}
                >
                  <span class="dir-icon">⌂</span>
                  <span class="dir-name">{dir}</span>
                </div>
              {/each}
            </div>
          {/if}
        {:else if parsed.cmd === 'f-file'}
          {#if !activeFileDir}
            <span class="text dim">no active file — open a PDF first</span>
          {:else if filteredPdfs.length === 0}
            <span class="text dim"
              >{parsed.query
                ? 'no matching PDFs'
                : 'no PDFs in active file directory'}</span
            >
          {:else}
            <span class="label">add file:</span>
            <div class="option-list" bind:this={optionListEl}>
              {#each filteredPdfs as pdf, i (pdf)}
                <div
                  class="option-row pdf-row"
                  class:sel={i === selectedOptionIdx}
                >
                  <span class="pdf-name">{basename(pdf)}</span>
                </div>
              {/each}
            </div>
          {/if}
        {:else if parsed.cmd === 'ba'}
          <span class="text"
            >delete all {app.files.length} buffer{app.files.length !== 1
              ? 's'
              : ''}</span
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

  /* Shared option list */
  .option-list {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-top: 2px;
    max-height: 180px;
    overflow-y: auto;
    scrollbar-width: none;
  }
  .option-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 6px;
    border-radius: 3px;
    color: var(--muted-hi);
    cursor: default;
    flex-shrink: 0;
  }
  .option-row.sel {
    background: var(--bg);
    color: var(--text);
  }

  /* Profile picker rows */
  .profile-cs {
    font-size: 9px;
    padding: 1px 5px;
    border-radius: 3px;
    flex-shrink: 0;
    font-weight: 700;
    letter-spacing: 0.05em;
  }
  .profile-cs.cmyk {
    background: #2a1500;
    color: #f97316;
    border: 1px solid #4a2500;
  }
  .profile-cs.rgb {
    background: #0f1e30;
    color: #60a5fa;
    border: 1px solid #1e3a5f;
  }
  .profile-label {
    flex: 1;
    font-size: 12px;
  }
  .option-row.sel .profile-label {
    color: var(--orange-hi);
  }
  .profile-value {
    font-size: 10px;
    color: var(--muted);
  }

  /* Directory picker rows */
  .dir-icon {
    font-size: 10px;
    color: var(--muted);
    flex-shrink: 0;
  }
  .dir-name {
    flex: 1;
    font-size: 12px;
  }
  .option-row.sel .dir-name {
    color: var(--orange-hi);
  }

  /* PDF picker rows */
  .pdf-name {
    font-size: 12px;
  }
  .option-row.sel .pdf-name {
    color: var(--orange-hi);
  }
</style>
