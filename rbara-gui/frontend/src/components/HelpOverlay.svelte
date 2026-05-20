<script>
  import { useAppState } from '../lib/context.js'
  const app = useAppState()

  let page = $state('shortcuts')
  let search = $state('')

  const shortcuts = [
    ['t', 'Trim Marks'],
    ['r', 'Resize to Bleed'],
    ['x', 'Export Images'],
    ['m', 'Remap Colors'],
    ['c', 'Convert Color Space'],
    ['s', 'Flatten Spot Colors'],
    ['b', 'Add Trim Box'],
    ['p', 'Split Pages'],
    ['g', 'Stitch Pages (exp)'],
    ['e', 'Extract Pages'],
    ['/', 'Output Path'],
    ['o', 'Toggle overwrite'],
    ['f', 'Add files…'],
    ['v', 'View active file in rbv'],
    ['a', 'Scope all files'],
    ['n', 'Scope no files'],
    ['i', 'Invert file scope'],
    ['Enter', 'Run active action'],
    ['?', 'Toggle help'],
    ['Esc', 'Close help / cancel'],
  ]

  const navShortcuts = [
    ['h / l', 'Move cursor left / right'],
    ['j / k', 'Move cursor down / up  (row-aware)'],
    ['Shift + H / L', 'Scope in current, move left / right'],
    ['Shift + J / K', 'Scope in current, move down / up'],
    ['Ctrl + i', 'Toggle active file scope'],
    ['Ctrl + t', 'Toggle Trim category expand'],
    ['Ctrl + p', 'Toggle Pages category expand'],
    ['Ctrl + c', 'Toggle Color category expand'],
    ['Ctrl + h / l', 'Scope out current, move left / right'],
    ['Ctrl + j / k', 'Scope out current, move down / up'],
  ]

  const rbvShortcuts = [
    ['Esc', 'Close viewer'],
    ['H  K  ←  ↑', 'Previous page'],
    ['L  J  →  ↓', 'Next page'],
    ['N + g', 'Jump to page N  (e.g. 5g)'],
  ]

  const cmdBarCommands = [
    { cmd: 'minimize / min / hide', desc: 'Minimize the rbara window' },
    { cmd: 'full / max / maximize', desc: 'Toggle maximize the rbara window' },
    {
      cmd: 'csrc::<query>',
      desc: 'Set color conversion source profile  (inline ICC search, ↑↓ to select, Tab to fill)',
    },
    {
      cmd: 'cdst::<query>',
      desc: 'Set color conversion destination profile  (inline ICC search, ↑↓ to select, Tab to fill)',
    },
    {
      cmd: '/n::<query>',
      desc: "Browse output folder starting from active file's directory  (↑↓ to navigate, Tab to drill in, Enter to confirm)",
    },
    {
      cmd: 'f::<query>',
      desc: "Add a PDF from active file's directory  (↑↓ to select, Tab to fill, Enter to add)",
    },
    {
      cmd: 'b(f64 | d)',
      desc: 'Set trim box bleed in inches. Bare :b shows current value. d resets to default (0.125 in). Also accepts bare shorthand: b0.125',
    },
    {
      cmd: 'r(f64 | d)',
      desc: 'Set resize bleed in inches. d resets to default (0.125 in).',
    },
    {
      cmd: 'e(pages | d)',
      desc: 'Set extract pages pattern — e.g. 1,3-5,7. d resets to default (1). Also accepts bare shorthand: e1,3-5',
    },
    {
      cmd: 'x  ·  x.fmt(format)  ·  x.dpi(n)',
      desc: 'Set export image format and/or DPI. Bare :x shows method list (↑↓ Tab to pick). Methods chain in any order; last value wins for duplicates. d resets a method to its default.',
    },
    {
      cmd: 'p.in(f64)  ·  p.mm(f64)',
      desc: 'Set split panel width. Bare :p shows unit picker (↑↓ Tab). mm values are auto-divided by 25.4. d resets to default (5.83 in).',
    },
    {
      cmd: 'g.in(f64)  ·  g.mm(f64)',
      desc: 'Set stitch spread width. Same unit options as :p. d resets to default (8.5 in).',
    },
    {
      cmd: 'm.src(C,M,Y,K).p | .d',
      desc: 'Set remap source CMYK colour. .p = values as 0–100 %, .d = values as 0.0–1.0. Bare :m shows method picker (↑↓ Tab). d as the argument resets to default with no modifier needed.',
    },
    {
      cmd: 'm.dst(C,M,Y,K).p | .d',
      desc: 'Set remap destination CMYK colour.',
    },
    {
      cmd: 'm.tol(n).p | .d',
      desc: 'Set remap match tolerance. 0 = exact pixel match only, 1 = remap every pixel.',
    },
    { cmd: 'bd', desc: 'Delete the first buffer' },
    { cmd: 'N bd', desc: 'Delete buffer N  (1-indexed, e.g. 2bd)' },
    { cmd: 'N-M bd', desc: 'Delete a range of buffers  (e.g. 1-3bd)' },
    { cmd: 'N,M,K bd', desc: 'Delete specific buffers  (e.g. 1,3,5bd)' },
    { cmd: 'ba', desc: 'Delete all buffers' },
    { cmd: 'sa', desc: 'Scope all buffers' },
    { cmd: 'sd', desc: 'Scope out all buffers' },
    { cmd: 'N-M sd', desc: 'Scope out a range of buffers  (e.g. 1-3sd)' },
    { cmd: 'N,M,K sd', desc: 'Scope out specific buffers  (e.g. 1,3,5sd)' },
    { cmd: 's', desc: 'Scope active file + next file' },
    { cmd: 'sN', desc: 'Scope only file N  (e.g. s2)' },
    { cmd: 'N-M s', desc: 'Scope in a range of buffers  (e.g. 1-3s)' },
    { cmd: 'N,M,K s', desc: 'Scope in specific buffers  (e.g. 1,3,5s)' },
    { cmd: 'v', desc: 'Open viewer for all scoped files' },
    { cmd: 'vN', desc: 'Open viewer for file N  (e.g. v2)' },
    { cmd: '/n', desc: 'Pick custom output folder' },
    { cmd: '/s', desc: 'Set output → same folder as source' },
    { cmd: 'theme', desc: 'Toggle dark / light theme' },
    { cmd: 'nq', desc: 'Load a fresh set of random quips' },
    { cmd: 'q / quit / exit', desc: 'Close rbara' },
  ]

  const chords = [
    {
      chord: 'Ctrl/Cmd + B  →  D',
      desc: 'Open command bar pre-filled with :bd',
    },
    {
      chord: 'Ctrl/Cmd + B  →  A',
      desc: 'Open command bar pre-filled with :ba',
    },
    { chord: 'Ctrl/Cmd + Q', desc: 'Refresh quip directly (no command bar)' },
    { chord: 'Ctrl/Cmd + /  →  N', desc: 'Pick custom output folder' },
    { chord: 'Ctrl/Cmd + /  →  S', desc: 'Set output → same folder as source' },
    { chord: 'Ctrl/Cmd + S  →  A', desc: 'Scope all files' },
    { chord: 'Ctrl/Cmd + S  →  D', desc: 'Scope out all files' },
    { chord: 'Ctrl/Cmd + S  →  [1-9]', desc: 'Scope only file at position N' },
    { chord: 'Ctrl/Cmd + S  (alone)', desc: 'Scope active file + next file' },
    { chord: 'Ctrl/Cmd + V', desc: 'Open :v command bar (viewer preview)' },
  ]

  const types = [
    {
      name: 'f64',
      title: 'Decimal number',
      desc: 'Any number, with or without a decimal point. Used for measurements in inches and for colour channel values.',
      examples: ['0.125', '5.83', '0.5', '1'],
    },
    {
      name: 'u32',
      title: 'Whole number',
      desc: 'A positive integer with no decimal point. Used for export DPI (dots per inch).',
      examples: ['72', '150', '300'],
    },
    {
      name: 'pages',
      title: 'Page range pattern',
      desc: 'Selects which pages to include. Mix individual numbers, comma-separated lists, and dash ranges freely.',
      examples: ['1', '1,3,5', '2-4', '1,3-5,7'],
    },
    {
      name: 'format',
      title: 'Image format',
      desc: 'The container format used when exporting pages as raster images. Only these four values are valid.',
      examples: ['jpg', 'png', 'webp', 'tiff'],
    },
    {
      name: 'C,M,Y,K',
      title: 'CMYK colour',
      desc: 'Four values for Cyan, Magenta, Yellow, and Black ink channels, separated by commas. Use the .p modifier for 0–100 % input or .d for 0.0–1.0 direct input — both map to the same internal colour.',
      examples: ['0,0,0,100  (.p → solid black)', '0.6,0.4,0.2,1.0  (.d)'],
    },
    {
      name: 'n  (tolerance)',
      title: 'Tolerance value',
      desc: 'A decimal controlling how loosely a pixel must match the source colour before being remapped. 0 = exact match only; 1 = remap every pixel regardless of colour.',
      examples: ['0', '0.05', '0.5', '1'],
    },
    {
      name: 'd',
      title: 'Default keyword',
      desc: 'The literal letter d used as an argument value. Resets that parameter to its built-in default — no number required. Works in every command that accepts a numeric or text value.',
      examples: [':b(d)', ':r(d)', ':x.fmt(d).dpi(d)', ':m.src(d)'],
    },
    {
      name: '.p',
      title: 'Percentage modifier',
      desc: 'Appended after a CMYK method call. Interprets the four channel values as 0–100 % and converts them to 0.0–1.0 before saving.',
      examples: [':m.src(100,0,0,0).p'],
    },
    {
      name: '.d',
      title: 'Direct modifier',
      desc: 'Appended after a CMYK method call. Interprets the four channel values as 0.0–1.0 directly — no conversion is applied.',
      examples: [':m.dst(0.6,0.4,0.2,1.0).d'],
    },
  ]

  let q = $derived(search.trim().toLowerCase())

  let filteredCmds = $derived(
    q
      ? cmdBarCommands.filter(
          (c) =>
            c.cmd.toLowerCase().includes(q) || c.desc.toLowerCase().includes(q),
        )
      : cmdBarCommands,
  )

  let filteredChords = $derived(
    q
      ? chords.filter(
          (c) =>
            c.chord.toLowerCase().includes(q) ||
            c.desc.toLowerCase().includes(q),
        )
      : chords,
  )

  let noResults = $derived(
    page === 'cmdbar' &&
      q &&
      filteredCmds.length === 0 &&
      filteredChords.length === 0,
  )

  function close() {
    app.helpVisible = false
    page = 'shortcuts'
    search = ''
  }
</script>

<div class="overlay" onclick={close} role="presentation">
  <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog">
    <div class="tabs">
      <button
        class="tab"
        class:active={page === 'shortcuts'}
        onclick={() => {
          page = 'shortcuts'
          search = ''
        }}>Shortcuts</button
      >
      <button
        class="tab"
        class:active={page === 'cmdbar'}
        onclick={() => (page = 'cmdbar')}>Command Bar</button
      >
      <button
        class="tab"
        class:active={page === 'types'}
        onclick={() => {
          page = 'types'
          search = ''
        }}>Types</button
      >
    </div>

    {#if page === 'shortcuts'}
      <div class="grid">
        {#each shortcuts as [k, label]}
          <div class="key">{k}</div>
          <div class="label">{label}</div>
        {/each}
        <div class="grid-section-label">File Navigation</div>
        {#each navShortcuts as [k, label]}
          <div class="key">{k}</div>
          <div class="label">{label}</div>
        {/each}
        <div class="grid-section-label">rbv Viewer</div>
        {#each rbvShortcuts as [k, label]}
          <div class="key">{k}</div>
          <div class="label">{label}</div>
        {/each}
      </div>
    {:else if page === 'cmdbar'}
      <p class="desc">
        Press <kbd>:</kbd> anywhere to enter command mode. Type a command and
        press
        <kbd>Enter</kbd> to execute — a live preview highlights affected buffers
        before you confirm. Press <kbd>Esc</kbd> to cancel at any time.
        <br /><br />
        Chord shortcuts (e.g. <kbd>Ctrl+B</kbd> then <kbd>D</kbd>) pre-fill the
        command bar so you always get a preview before anything is deleted.
      </p>

      <div class="search-row">
        <span class="search-icon">⌕</span>
        <input
          class="search"
          type="text"
          placeholder="Search commands…"
          bind:value={search}
          spellcheck="false"
          autocomplete="off"
        />
        {#if search}
          <button class="search-clear" onclick={() => (search = '')}>×</button>
        {/if}
      </div>

      {#if noResults}
        <div class="no-results">No matching commands for "{search}"</div>
      {:else}
        {#if filteredCmds.length > 0}
          <div class="section-label">Commands</div>
          <div class="cmd-grid">
            {#each filteredCmds as c}
              <div class="cmd-key">:{c.cmd}</div>
              <div class="cmd-desc">{c.desc}</div>
            {/each}
          </div>
        {/if}

        {#if filteredChords.length > 0}
          <div class="section-label">Chord Shortcuts</div>
          <div class="cmd-grid">
            {#each filteredChords as c}
              <div class="cmd-key chord">{c.chord}</div>
              <div class="cmd-desc">{c.desc}</div>
            {/each}
          </div>
        {/if}
      {/if}
    {:else if page === 'types'}
      <div class="types-list">
        {#each types as t}
          <div class="type-card">
            <div class="type-header">
              <code class="type-name">{t.name}</code>
              <span class="type-title">{t.title}</span>
            </div>
            <p class="type-desc">{t.desc}</p>
            <div class="type-examples">
              {#each t.examples as ex}
                <code class="type-ex">{ex}</code>
              {/each}
            </div>
          </div>
        {/each}
      </div>
    {/if}

    <button class="close" onclick={close}>Close</button>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: #000a;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  .modal {
    background: var(--surface);
    border: 1px solid var(--border-hi);
    border-radius: 8px;
    padding: 0 0 16px;
    width: 480px;
    max-width: 94vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 10px 40px #000a;
    overflow: hidden;
  }

  /* tabs */
  .tabs {
    display: flex;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .tab {
    flex: 1;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    padding: 10px 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted-hi);
    cursor: pointer;
    transition: 0.1s;
    letter-spacing: 0.03em;
  }
  .tab:hover {
    color: var(--text);
    background: var(--panel);
  }
  .tab.active {
    color: var(--orange);
    border-bottom-color: var(--orange);
  }

  /* shortcuts page */
  .grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 8px 18px;
    padding: 16px 20px;
    overflow-y: auto;
  }
  .grid-section-label {
    grid-column: 1 / -1;
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
    padding: 10px 0 2px;
    border-top: 1px solid var(--border);
    margin-top: 2px;
  }
  .key {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    padding: 2px 8px;
    border-radius: 3px;
    text-align: center;
    align-self: center;
  }
  .label {
    font-size: 12px;
    color: var(--muted-hi);
    align-self: center;
  }

  /* cmdbar page */
  .desc {
    font-size: 11.5px;
    color: var(--muted-hi);
    line-height: 1.6;
    padding: 14px 20px 10px;
    margin: 0;
    flex-shrink: 0;
  }
  kbd {
    font-family: var(--mono);
    font-size: 10.5px;
    background: var(--bg);
    border: 1px solid var(--border-hi);
    border-radius: 3px;
    padding: 1px 5px;
    color: var(--orange-hi);
  }
  .search-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 20px 10px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 5px 10px;
    flex-shrink: 0;
  }
  .search-icon {
    color: var(--muted);
    font-size: 14px;
    user-select: none;
  }
  .search {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    font-size: 12px;
    color: var(--text);
    font-family: var(--sans);
  }
  .search::placeholder {
    color: var(--muted);
  }
  .search-clear {
    background: none;
    border: none;
    color: var(--muted);
    font-size: 14px;
    cursor: pointer;
    padding: 0 2px;
    line-height: 1;
  }
  .search-clear:hover {
    color: var(--text);
  }
  .section-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
    padding: 6px 20px 4px;
    flex-shrink: 0;
  }
  .cmd-grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 14px;
    padding: 4px 20px 8px;
    overflow-y: auto;
  }
  .cmd-key {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--orange-hi);
    background: var(--bg);
    border: 1px solid var(--border);
    padding: 2px 8px;
    border-radius: 3px;
    white-space: nowrap;
    align-self: center;
  }
  .cmd-key.chord {
    color: var(--text);
    font-size: 10px;
  }
  .cmd-desc {
    font-size: 11.5px;
    color: var(--muted-hi);
    align-self: center;
  }
  .no-results {
    padding: 20px;
    text-align: center;
    color: var(--muted);
    font-size: 12px;
    font-style: italic;
  }

  /* types page */
  .types-list {
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }
  .type-card {
    padding: 11px 20px;
    border-bottom: 1px solid var(--border);
  }
  .type-card:last-child {
    border-bottom: none;
  }
  .type-header {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 4px;
  }
  .type-name {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--orange-hi);
    background: var(--bg);
    border: 1px solid var(--border);
    padding: 1px 6px;
    border-radius: 3px;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .type-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
  }
  .type-desc {
    font-size: 11.5px;
    color: var(--muted-hi);
    line-height: 1.55;
    margin: 0 0 7px;
  }
  .type-examples {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .type-ex {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--orange);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 1px 5px;
  }

  /* footer */
  .close {
    margin: 8px 20px 0;
    align-self: flex-end;
    background: var(--orange);
    color: #fff;
    border: none;
    border-radius: 5px;
    padding: 6px 16px;
    font-weight: 700;
    font-size: 12px;
    flex-shrink: 0;
  }
  .close:hover {
    background: var(--orange-hi);
  }
</style>
