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
    { cmd: 'csrc::<query>', desc: 'Set color conversion source profile  (inline ICC search, ↑↓ to select)' },
    { cmd: 'cdst::<query>', desc: 'Set color conversion destination profile  (inline ICC search, ↑↓ to select)' },
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
    { chord: 'Ctrl/Cmd + B  →  D', desc: 'Open command bar pre-filled with :bd' },
    { chord: 'Ctrl/Cmd + B  →  A', desc: 'Open command bar pre-filled with :ba' },
    { chord: 'Ctrl/Cmd + Q', desc: 'Refresh quip directly (no command bar)' },
    { chord: 'Ctrl/Cmd + /  →  N', desc: 'Pick custom output folder' },
    { chord: 'Ctrl/Cmd + /  →  S', desc: 'Set output → same folder as source' },
    { chord: 'Ctrl/Cmd + S  →  A', desc: 'Scope all files' },
    { chord: 'Ctrl/Cmd + S  →  D', desc: 'Scope out all files' },
    { chord: 'Ctrl/Cmd + S  →  [1-9]', desc: 'Scope only file at position N' },
    { chord: 'Ctrl/Cmd + S  (alone)', desc: 'Scope active file + next file' },
    { chord: 'Ctrl/Cmd + V', desc: 'Open :v command bar (viewer preview)' },
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
    {:else}
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
