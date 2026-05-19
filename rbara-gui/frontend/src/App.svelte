<script>
  import { onMount } from 'svelte'
  import { provideAppState } from './lib/context.js'
  import * as api from './lib/api.js'
  import { randomQuip } from './lib/quips.js'
  import Titlebar from './components/Titlebar.svelte'
  import Toolbar from './components/Toolbar.svelte'
  import FileStrip from './components/FileStrip.svelte'
  import ActionSidebar from './components/ActionSidebar.svelte'
  import ActionTabBar from './components/ActionTabBar.svelte'
  import ActionBar from './components/ActionBar.svelte'
  import MetaGrid from './components/MetaGrid.svelte'
  import MetaAccordion from './components/MetaAccordion.svelte'
  import PreviewStrip from './components/PreviewStrip.svelte'
  import ParamsPanel from './components/ParamsPanel.svelte'
  import LogPane from './components/LogPane.svelte'
  import LogDrawer from './components/LogDrawer.svelte'
  import LogStrip from './components/LogStrip.svelte'
  import StatusBar from './components/StatusBar.svelte'
  import HelpOverlay from './components/HelpOverlay.svelte'
  import CmdBar from './components/CmdBar.svelte'

  // ---------- core state ----------
  let files = $state([]) // [{path, name, colorSpace, sizeKb}]
  let activeFile = $state(null) // index | null
  let metadata = $state(null)
  let activeAction = $state('trim') // 'trim' | 'resize' | 'export' | 'remap' | 'output'
  let processing = $state(false)
  let overwrite = $state(false)
  let outputDir = $state(null)
  let actionLog = $state([]) // {ok, message, output_paths, timestamp, action}
  let helpVisible = $state(false)
  let theme = $state(localStorage.getItem('rbara-theme') ?? 'dark')
  let cmdBarVisible = $state(false)
  let cmdBarInput = $state('')
  let chordPending = $state(null)
  let chordTimer = null
  let dragOver = $state(false)
  let quip = $state(randomQuip())

  $effect(() => {
    document.body.classList.toggle('light', theme === 'light')
    localStorage.setItem('rbara-theme', theme)
  })

  let customProfiles = $state([]) // [{name, description, color_space}]

  let params = $state({
    bleedInches: 0.125,
    exportFormat: 'jpg',
    exportDpi: 150,
    remapFrom: [1.0, 1.0, 1.0, 1.0],
    remapTo: [0.6, 0.4, 0.2, 1.0],
    remapTolerance: 1.0,
    fromProfile: 'AdobeRGB1998',
    toProfile: 'USWebCoatedSWOP',
    convertIntent: 'RelativeColorimetric',
    trimBoxBleedInches: 0.125,
    extractPagesInput: '1',
    splitPanelInches: 5.83,
    stitchSpreadInches: 8.5,
  })

  // ---------- layout state ----------
  let windowWidth = $state(window.innerWidth)
  let windowHeight = $state(window.innerHeight)
  let layout = $derived(
    windowWidth > windowHeight * 1.4
      ? 'wide'
      : windowHeight > windowWidth
        ? 'vertical'
        : 'square',
  )

  // ---------- preflight ----------
  let canTrim = $derived(metadata?.has_trimbox ?? false)
  let canResize = $derived(metadata?.has_trimbox ?? false)
  let isMixedCs = $derived(metadata?.color_space === 'Mixed')
  let isPureRgb = $derived(metadata?.color_space === 'PureRGB')

  // ---------- output hint ----------
  let activeFileObj = $derived(
    activeFile !== null && files[activeFile] ? files[activeFile] : null,
  )
  let scopedFiles = $derived(files.filter((f) => f.scoped))
  let scopedPaths = $derived(scopedFiles.map((f) => f.path))
  let scopedCount = $derived(scopedFiles.length)
  let outputHint = $derived.by(() => {
    if (!activeFileObj) return ''
    const stem = activeFileObj.name.replace(/\.pdf$/i, '')
    const dir = outputDir ? outputDir + '/' : ''
    if (activeAction === 'splitpages') return `→ ${dir}${stem}_split.pdf`
    if (activeAction === 'stitchpages') return `→ ${dir}${stem}_stitch.pdf`
    if (overwrite) return activeFileObj.name
    const ext = activeAction === 'export' ? params.exportFormat : 'pdf'
    const suffix = activeAction === 'export' ? '_processed_1' : '_processed'
    return `→ ${dir}${stem}${suffix}.${ext}`
  })

  // ---------- file loading ----------
  async function addFilesFromPaths(paths) {
    for (const path of paths) {
      if (files.some((f) => f.path === path)) continue
      try {
        const meta = await api.loadMetadata(path)
        files = [
          ...files,
          {
            path,
            name: api.basename(path),
            colorSpace: meta.color_space,
            sizeKb: meta.file_size_kb,
            metadata: meta,
            scoped: true,
          },
        ]
      } catch (e) {
        actionLog = [
          {
            ok: false,
            message: typeof e === 'string' ? e : String(e),
            output_paths: [],
            timestamp: new Date().toLocaleTimeString(),
            action: 'LoadMetadata',
          },
          ...actionLog,
        ]
      }
    }
    if (activeFile === null && files.length > 0) {
      selectFile(0)
    }
  }

  async function addFiles() {
    let paths
    try {
      paths = await api.pickPdfFiles()
    } catch (e) {
      console.error(e)
      return
    }
    if (!paths.length) return
    await addFilesFromPaths(paths)
  }

  function removeFile(idx) {
    files = files.filter((_, i) => i !== idx)
    if (files.length === 0) {
      activeFile = null
      metadata = null
    } else if (activeFile === idx) {
      selectFile(0)
    } else if (activeFile !== null && activeFile > idx) {
      activeFile -= 1
    }
  }

  function selectFile(idx) {
    activeFile = idx
    metadata = files[idx]?.metadata ?? null
  }

  function clearAll() {
    files = []
    activeFile = null
    metadata = null
  }

  function openCmdBar(initial = '') {
    cmdBarInput = initial
    cmdBarVisible = true
  }

  function closeCmdBar() {
    cmdBarVisible = false
    cmdBarInput = ''
  }

  function refreshQuip() {
    quip = randomQuip()
  }

  async function executeCmdBar(parsed) {
    if (parsed.cmd === 'ba') {
      clearAll()
    } else if (parsed.cmd === 'minimize') {
      closeCmdBar()
      api.minimizeWindow().catch(console.error)
      return
    } else if (parsed.cmd === 'maximize') {
      closeCmdBar()
      api.toggleMaximizeWindow().catch(console.error)
      return
    } else if (parsed.cmd === 'nq') {
      refreshQuip()
    } else if (parsed.cmd === 'exit') {
      closeCmdBar()
      api.exitApp().catch(() => window.close())
      return
    } else if (parsed.cmd === 'theme') {
      theme = theme === 'dark' ? 'light' : 'dark'
    } else if (parsed.cmd === '/n') {
      closeCmdBar()
      try {
        const dir = await api.pickOutputDir()
        if (dir) outputDir = dir
      } catch (e) { console.error(e) }
      return
    } else if (parsed.cmd === '/s') {
      outputDir = null
    } else if (parsed.cmd === 'sa') {
      scopeAll()
    } else if (parsed.cmd === 'sd') {
      scopeNone()
    } else if (parsed.cmd === 's') {
      if (parsed.index != null) {
        files = files.map((f, i) => ({ ...f, scoped: i === parsed.index }))
      } else {
        const base = activeFile ?? 0
        files = files.map((f, i) => ({ ...f, scoped: i === base || i === base + 1 }))
      }
    } else if (parsed.cmd === 'v') {
      const targets = parsed.index != null
        ? [files[parsed.index]].filter(Boolean)
        : files.filter(f => f.scoped)
      targets.forEach(f => api.openInViewer(f.path).catch(console.error))
    } else if (parsed.cmd === 'bd') {
      const sorted = [...parsed.indices].sort((a, b) => b - a)
      for (const idx of sorted) removeFile(idx)
    }
    closeCmdBar()
  }

  function toggleScope(idx) {
    files = files.map((f, i) => (i === idx ? { ...f, scoped: !f.scoped } : f))
  }
  function scopeIn(idx) {
    files = files.map((f, i) => (i === idx ? { ...f, scoped: true } : f))
  }
  function scopeOut(idx) {
    files = files.map((f, i) => (i === idx ? { ...f, scoped: false } : f))
  }
  function scopeAll() {
    files = files.map((f) => ({ ...f, scoped: true }))
  }
  function scopeNone() {
    files = files.map((f) => ({ ...f, scoped: false }))
  }
  function invertScope() {
    files = files.map((f) => ({ ...f, scoped: !f.scoped }))
  }

  // ---------- vim-style file navigation ----------
  function navigateVisual(idx, dir) {
    const chips = Array.from(document.querySelectorAll('.file-chip'))
    if (!chips[idx] || chips.length === 0) return idx
    const rects = chips.map((el) => el.getBoundingClientRect())
    const curTop = Math.round(rects[idx].top)
    const centerX = rects[idx].left + rects[idx].width / 2
    const tops = rects.map((r) => Math.round(r.top))
    if (dir === 'j') {
      const nextTop = tops.filter((t) => t > curTop).reduce((min, t) => Math.min(min, t), Infinity)
      if (nextTop === Infinity) return idx
      return tops.reduce((best, t, i) => {
        if (t !== nextTop) return best
        const dist = Math.abs(rects[i].left + rects[i].width / 2 - centerX)
        if (best === -1) return i
        return dist < Math.abs(rects[best].left + rects[best].width / 2 - centerX) ? i : best
      }, -1)
    } else {
      const prevTop = tops.filter((t) => t < curTop).reduce((max, t) => Math.max(max, t), -Infinity)
      if (prevTop === -Infinity) return idx
      return tops.reduce((best, t, i) => {
        if (t !== prevTop) return best
        const dist = Math.abs(rects[i].left + rects[i].width / 2 - centerX)
        if (best === -1) return i
        return dist < Math.abs(rects[best].left + rects[best].width / 2 - centerX) ? i : best
      }, -1)
    }
  }

  function navigateFile(dir, scopeAction = 'none') {
    if (files.length === 0 || activeFile === null) return
    if (scopeAction === 'in') scopeIn(activeFile)
    else if (scopeAction === 'out') scopeOut(activeFile)
    let newIdx
    if (dir === 'h') newIdx = Math.max(0, activeFile - 1)
    else if (dir === 'l') newIdx = Math.min(files.length - 1, activeFile + 1)
    else newIdx = navigateVisual(activeFile, dir)
    selectFile(newIdx)
    document.querySelectorAll('.file-chip')[newIdx]?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  }

  // ---------- sidebar category expand state ----------
  const TRIM_IDS = new Set(['trim', 'addtrimbox'])
  const PAGES_IDS = new Set(['splitpages', 'stitchpages', 'extractpages'])
  const COLOR_IDS = new Set(['remap', 'colorspace', 'spots'])

  let trimExpanded = $state(TRIM_IDS.has(activeAction))
  let pagesExpanded = $state(PAGES_IDS.has(activeAction))
  let colorExpanded = $state(COLOR_IDS.has(activeAction))

  $effect(() => {
    if (TRIM_IDS.has(activeAction)) trimExpanded = true
    if (PAGES_IDS.has(activeAction)) pagesExpanded = true
    if (COLOR_IDS.has(activeAction)) colorExpanded = true
  })

  // ---------- run actions ----------
  async function replaceProcessedFiles(outputPaths) {
    const scopedIndices = files.map((f, i) => (f.scoped ? i : -1)).filter((i) => i !== -1)
    const updated = [...files]
    for (let j = 0; j < scopedIndices.length && j < outputPaths.length; j++) {
      const idx = scopedIndices[j]
      const newPath = outputPaths[j]
      try {
        const meta = await api.loadMetadata(newPath)
        updated[idx] = {
          path: newPath,
          name: api.basename(newPath),
          colorSpace: meta.color_space,
          sizeKb: meta.file_size_kb,
          metadata: meta,
          scoped: true,
        }
      } catch {
        // keep original entry if the output file can't be read
      }
    }
    files = updated
    if (activeFile !== null) {
      metadata = files[activeFile]?.metadata ?? null
    }
  }

  async function runAction() {
    if (processing || files.length === 0) return
    if (activeAction === 'output') return

    const paths = files.filter((f) => f.scoped).map((f) => f.path)
    if (paths.length === 0) return
    processing = true

    try {
      let result
      let actionLabel
      switch (activeAction) {
        case 'trim':
          actionLabel = 'TrimMarks'
          result = await api.trimMarks(paths, outputDir, overwrite)
          break
        case 'resize':
          actionLabel = 'ResizeToBleed'
          result = await api.resizeToBleed(
            paths,
            params.bleedInches,
            outputDir,
            overwrite,
          )
          break
        case 'export':
          actionLabel = 'ExportImages'
          result = await api.exportImages(
            paths,
            params.exportFormat,
            params.exportDpi,
            outputDir,
          )
          break
        case 'remap':
          actionLabel = 'RemapColors'
          result = await api.remapColors(
            paths,
            params.remapFrom,
            params.remapTo,
            params.remapTolerance,
            outputDir,
            overwrite,
          )
          break
        case 'colorspace':
          actionLabel = 'ConvertColorSpace'
          result = await api.convertColorSpace(
            paths,
            params.fromProfile,
            params.toProfile,
            params.convertIntent,
            outputDir,
            overwrite,
          )
          break
        case 'spots':
          actionLabel = 'FlattenSpots'
          result = await api.flattenSpots(paths, outputDir, overwrite)
          break
        case 'addtrimbox':
          actionLabel = 'AddTrimBox'
          result = await api.addTrimBox(
            paths,
            params.trimBoxBleedInches,
            outputDir,
            overwrite,
          )
          break
        case 'splitpages':
          actionLabel = 'SplitPages'
          result = await api.splitPages(paths, params.splitPanelInches * 72, outputDir, overwrite)
          break
        case 'stitchpages':
          actionLabel = 'StitchPages'
          result = await api.stitchPages(paths, params.stitchSpreadInches * 72, outputDir, overwrite)
          break
        case 'extractpages': {
          actionLabel = 'ExtractPages'
          const pageNums = api.parsePageNums(params.extractPagesInput)
          result = await api.extractPages(paths, pageNums, outputDir, overwrite)
          break
        }
        default:
          processing = false
          return
      }
      actionLog = [{ ...result, action: actionLabel }, ...actionLog]

      const SWAP_ACTIONS = new Set([
        'trim', 'resize', 'remap', 'colorspace', 'spots', 'addtrimbox', 'extractpages', 'splitpages', 'stitchpages',
      ])
      if (SWAP_ACTIONS.has(activeAction) && result.output_paths.length > 0) {
        await replaceProcessedFiles(result.output_paths)
      }
    } catch (e) {
      actionLog = [
        {
          ok: false,
          message: typeof e === 'string' ? e : String(e),
          output_paths: [],
          timestamp: new Date().toLocaleTimeString(),
          action: activeAction,
        },
        ...actionLog,
      ]
    } finally {
      processing = false
    }
  }

  // ---------- keyboard shortcuts ----------
  function handleKey(e) {
    const tag = document.activeElement?.tagName
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return
    if (cmdBarVisible) return

    // Ctrl/Cmd+B chord initiation
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === 'b') {
      clearTimeout(chordTimer)
      chordPending = 'b'
      chordTimer = setTimeout(() => { chordPending = null }, 2000)
      e.preventDefault()
      return
    }

    // Ctrl/Cmd+Q — refresh quip directly
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === 'q') {
      refreshQuip()
      e.preventDefault()
      return
    }

    // Ctrl/Cmd+/ chord: output path
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key === '/') {
      clearTimeout(chordTimer)
      chordPending = '/'
      chordTimer = setTimeout(() => { chordPending = null }, 2000)
      e.preventDefault()
      return
    }

    // Ctrl/Cmd+S chord: scope (fires scope current+next after 800ms if no follow-up)
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === 's') {
      clearTimeout(chordTimer)
      chordPending = 's'
      chordTimer = setTimeout(() => {
        chordPending = null
        if (files.length === 0) return
        const base = activeFile ?? 0
        files = files.map((f, i) => ({ ...f, scoped: i === base || i === base + 1 }))
      }, 800)
      e.preventDefault()
      return
    }

    // Ctrl/Cmd+V: open viewer cmdbar for scoped files
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === 'v') {
      openCmdBar('v')
      e.preventDefault()
      return
    }

    // Chord completion: d → delete buffer, a → delete all
    if (chordPending === 'b') {
      clearTimeout(chordTimer)
      chordTimer = null
      chordPending = null
      const k = e.key.toLowerCase()
      if (k === 'd') { openCmdBar('bd'); e.preventDefault(); return }
      if (k === 'a') { openCmdBar('ba'); e.preventDefault(); return }
    }

    // Chord completion for /: n → pick custom dir, s → same as source
    if (chordPending === '/') {
      clearTimeout(chordTimer)
      chordTimer = null
      chordPending = null
      const k = e.key.toLowerCase()
      if (k === 'n') {
        api.pickOutputDir().then(dir => { if (dir) outputDir = dir }).catch(console.error)
        e.preventDefault(); return
      }
      if (k === 's') { outputDir = null; e.preventDefault(); return }
    }

    // Chord completion for s: a → scope all, d → scope none, digit → scope file N
    if (chordPending === 's') {
      clearTimeout(chordTimer)
      chordTimer = null
      chordPending = null
      if (files.length > 0) {
        const k = e.key.toLowerCase()
        if (k === 'a') { scopeAll(); e.preventDefault(); return }
        if (k === 'd') { scopeNone(); e.preventDefault(); return }
        if (/^\d$/.test(e.key)) {
          const idx = parseInt(e.key, 10) - 1
          if (idx >= 0) files = files.map((f, i) => ({ ...f, scoped: i === idx }))
          e.preventDefault(); return
        }
        const base = activeFile ?? 0
        files = files.map((f, i) => ({ ...f, scoped: i === base || i === base + 1 }))
      }
    }

    // Ctrl/Cmd+h/l/j/k: scope out current file, then navigate
    // Ctrl/Cmd+i: toggle active file scope
    // Ctrl/Cmd+t/p/c: toggle sidebar category expand
    if ((e.ctrlKey || e.metaKey) && !e.altKey) {
      const k = e.key.toLowerCase()
      const navDir = { h: 'h', l: 'l', j: 'j', k: 'k', arrowleft: 'h', arrowright: 'l', arrowdown: 'j', arrowup: 'k' }[k]
      if (navDir && files.length > 0) {
        clearTimeout(chordTimer)
        chordTimer = null
        chordPending = null
        navigateFile(navDir, 'out')
        e.preventDefault()
        return
      }
      if (k === 'i' && activeFile !== null) {
        toggleScope(activeFile)
        e.preventDefault()
        return
      }
      if (k === 't' || k === 'p' || k === 'c') {
        clearTimeout(chordTimer)
        chordTimer = null
        chordPending = null
        if (k === 't') trimExpanded = !trimExpanded
        else if (k === 'p') pagesExpanded = !pagesExpanded
        else colorExpanded = !colorExpanded
        e.preventDefault()
        return
      }
    }

    if (e.ctrlKey || e.metaKey || e.altKey) return

    switch (e.key) {
      case 'h':
        navigateFile('h')
        break
      case 'l':
        navigateFile('l')
        break
      case 'j':
        navigateFile('j')
        break
      case 'k':
        navigateFile('k')
        break
      case 'ArrowLeft':
        navigateFile('h', e.shiftKey ? 'in' : 'none')
        break
      case 'ArrowRight':
        navigateFile('l', e.shiftKey ? 'in' : 'none')
        break
      case 'ArrowDown':
        navigateFile('j', e.shiftKey ? 'in' : 'none')
        break
      case 'ArrowUp':
        navigateFile('k', e.shiftKey ? 'in' : 'none')
        break
      case 'H':
        navigateFile('h', 'in')
        break
      case 'L':
        navigateFile('l', 'in')
        break
      case 'J':
        navigateFile('j', 'in')
        break
      case 'K':
        navigateFile('k', 'in')
        break
      case ':':
        openCmdBar('')
        break
      case 't':
        activeAction = 'trim'
        break
      case 'r':
        activeAction = 'resize'
        break
      case 'x':
        activeAction = 'export'
        break
      case 'm':
        activeAction = 'remap'
        break
      case 'c':
        activeAction = 'colorspace'
        break
      case 's':
        activeAction = 'spots'
        break
      case 'b':
        activeAction = 'addtrimbox'
        break
      case 'p':
        activeAction = 'splitpages'
        break
      case 'g':
        activeAction = 'stitchpages'
        break
      case 'e':
        activeAction = 'extractpages'
        break
      case '/':
        activeAction = 'output'
        break
      case 'o':
        overwrite = !overwrite
        break
      case 'f':
        addFiles()
        break
      case 'v':
        if (activeFileObj) api.openInViewer(activeFileObj.path).catch(console.error)
        break
      case 'a':
        scopeAll()
        break
      case 'n':
        scopeNone()
        break
      case 'i':
        invertScope()
        break
      case '?':
        helpVisible = !helpVisible
        break
      case 'Escape':
        if (helpVisible) helpVisible = false
        break
      case 'Enter':
        if (files.length > 0 && !processing) runAction()
        break
      default:
        return
    }
    e.preventDefault()
  }

  onMount(() => {
    const onResize = () => {
      windowWidth = window.innerWidth
      windowHeight = window.innerHeight
    }
    window.addEventListener('resize', onResize)
    document.addEventListener('keydown', handleKey)

    api
      .listCustomProfiles()
      .then((saved) => {
        customProfiles = saved
      })
      .catch(() => {})

    let unlistenDrop = null
    import('@tauri-apps/api/webviewWindow')
      .then(({ getCurrentWebviewWindow }) =>
        getCurrentWebviewWindow().onDragDropEvent((event) => {
          if (event.payload.type === 'hover') {
            dragOver = true
          } else if (event.payload.type === 'drop') {
            dragOver = false
            const pdfPaths = event.payload.paths.filter((p) =>
              p.toLowerCase().endsWith('.pdf'),
            )
            if (pdfPaths.length > 0) addFilesFromPaths(pdfPaths)
          } else {
            dragOver = false
          }
        }),
      )
      .then((fn) => { unlistenDrop = fn })
      .catch(() => {})

    return () => {
      window.removeEventListener('resize', onResize)
      document.removeEventListener('keydown', handleKey)
      if (unlistenDrop) unlistenDrop()
    }
  })

  // ---------- expose to children ----------
  provideAppState({
    get files() {
      return files
    },
    get activeFile() {
      return activeFile
    },
    get activeFileObj() {
      return activeFileObj
    },
    get metadata() {
      return metadata
    },
    get activeAction() {
      return activeAction
    },
    set activeAction(v) {
      activeAction = v
    },
    get processing() {
      return processing
    },
    get overwrite() {
      return overwrite
    },
    set overwrite(v) {
      overwrite = v
    },
    get outputDir() {
      return outputDir
    },
    set outputDir(v) {
      outputDir = v
    },
    get actionLog() {
      return actionLog
    },
    get params() {
      return params
    },
    get customProfiles() {
      return customProfiles
    },
    addCustomProfile(p) {
      customProfiles = [...customProfiles, p]
    },
    logAction(entry) {
      actionLog = [entry, ...actionLog]
    },
    get layout() {
      return layout
    },
    get canTrim() {
      return canTrim
    },
    get canResize() {
      return canResize
    },
    get isMixedCs() {
      return isMixedCs
    },
    get isPureRgb() {
      return isPureRgb
    },
    get outputHint() {
      return outputHint
    },
    get helpVisible() {
      return helpVisible
    },
    set helpVisible(v) {
      helpVisible = v
    },
    get theme() {
      return theme
    },
    set theme(v) {
      theme = v
    },
    get cmdBarVisible() {
      return cmdBarVisible
    },
    get cmdBarInput() {
      return cmdBarInput
    },
    set cmdBarInput(v) {
      cmdBarInput = v
    },
    openCmdBar,
    closeCmdBar,
    executeCmdBar,
    get quip() {
      return quip
    },
    refreshQuip,
    get scopedFiles() {
      return scopedFiles
    },
    get scopedPaths() {
      return scopedPaths
    },
    get scopedCount() {
      return scopedCount
    },
    addFiles,
    removeFile,
    selectFile,
    clearAll,
    runAction,
    toggleScope,
    scopeIn,
    scopeOut,
    scopeAll,
    scopeNone,
    invertScope,
    get trimExpanded() { return trimExpanded },
    set trimExpanded(v) { trimExpanded = v },
    get pagesExpanded() { return pagesExpanded },
    set pagesExpanded(v) { pagesExpanded = v },
    get colorExpanded() { return colorExpanded },
    set colorExpanded(v) { colorExpanded = v },
  })
</script>

<Titlebar />

{#if layout === 'wide'}
  <Toolbar />
  <FileStrip />
  <div class="app-body">
    <ActionSidebar />
    <div class="center-pane">
      <PreviewStrip />
      <MetaGrid />
      <ParamsPanel />
    </div>
    <LogPane />
  </div>
{:else if layout === 'square'}
  <FileStrip />
  <ActionTabBar />
  <div class="center-pane fill">
    <MetaGrid compact />
    <ParamsPanel />
    <LogStrip />
  </div>
{:else}
  <FileStrip />
  <ActionBar />
  <div class="center-pane fill">
    <MetaAccordion />
    <ParamsPanel />
  </div>
  <LogDrawer />
{/if}

<StatusBar />

{#if dragOver}
  <div class="drag-overlay" aria-hidden="true">
    <div class="drag-box">Drop PDFs</div>
  </div>
{/if}

{#if helpVisible}
  <HelpOverlay />
{/if}

<CmdBar />

<style>
  .app-body {
    display: flex;
    flex: 1;
    overflow: hidden;
  }
  .center-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg);
    min-width: 0;
  }
  .center-pane.fill {
    flex: 1;
  }
  .drag-overlay {
    position: fixed;
    inset: 0;
    background: rgba(232, 104, 7, 0.1);
    border: 2px dashed var(--orange);
    pointer-events: none;
    z-index: 900;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .drag-box {
    background: var(--panel);
    border: 1px solid var(--orange);
    border-radius: 6px;
    padding: 10px 20px;
    font-size: 13px;
    color: var(--orange);
    font-family: var(--sans);
  }
</style>
