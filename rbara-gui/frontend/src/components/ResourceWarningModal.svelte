<script>
  import { useAppState } from '../lib/context.js'
  import { formatSize } from '../lib/api.js'
  const app = useAppState()

  // `file` is the rejected file: { name, sizeKb }. The file is NOT added to the
  // buffer — this modal only informs the user why and dismisses.
  let { file, onDismiss } = $props()

  const limitMb = app.settings?.resource_warn_size_mb ?? 200
</script>

<div class="overlay" role="presentation">
  <div class="modal" role="dialog" aria-modal="true">
    <div class="modal-header">
      <span class="warn-icon">⚡</span>
      <div>
        <div class="modal-title">Large File — Not Supported Yet</div>
        <div class="modal-sub">{file.name}</div>
      </div>
    </div>

    <div class="stats-row">
      <div class="stat">
        <span class="stat-val">{formatSize(file.sizeKb)}</span>
        <span class="stat-label">file size</span>
      </div>
    </div>

    <p class="modal-body">
      This file exceeds the {limitMb} MB limit and was
      <strong>not added</strong>. rustybara can't yet parse very large PDFs
      without freezing — splitting and re-merging them was explored but didn't
      hold up, so for now oversized files are refused rather than processed
      unreliably. See <strong>Known Limitations</strong>
      in the
      <a
        href="https://github.com/Addy-A/rustybara#known-limitations"
        target="_blank">README</a
      >.
    </p>

    <p class="modal-body subtle">
      You can raise or disable the limit in <strong>Settings → Behavior</strong
      >, but files much larger than this will likely hang the app.
    </p>

    <div class="modal-footer">
      <button class="btn-dismiss" onclick={onDismiss}>Got it</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: #000c;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1100;
  }
  .modal {
    background: var(--surface);
    border: 1px solid var(--border-hi);
    border-radius: 8px;
    padding: 20px 22px;
    width: 420px;
    max-width: 94vw;
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: 0 12px 48px #0008;
  }
  .modal-header {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .warn-icon {
    font-size: 24px;
    color: var(--warn);
    flex-shrink: 0;
  }
  .modal-title {
    font-size: 14px;
    font-weight: 700;
    color: var(--text);
  }
  .modal-sub {
    font-size: 11px;
    color: var(--muted-hi);
    margin-top: 2px;
    font-family: var(--mono);
  }

  .stats-row {
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px 14px;
  }
  .stat {
    display: flex;
    flex-direction: column;
    align-items: center;
    flex: 1;
  }
  .stat-val {
    font-size: 18px;
    font-weight: 700;
    color: var(--orange);
    font-family: var(--mono);
  }
  .stat-label {
    font-size: 10px;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .modal-body {
    font-size: 12px;
    color: var(--muted-hi);
    line-height: 1.6;
    margin: 0;
  }
  .modal-body.subtle {
    font-size: 11px;
    color: var(--muted);
  }
  .modal-body strong {
    color: var(--text);
  }

  .modal-footer {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    padding-top: 4px;
  }
  .btn-dismiss {
    padding: 7px 18px;
    background: var(--orange);
    border: none;
    border-radius: 5px;
    color: #fff;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-dismiss:hover {
    opacity: 0.9;
  }
</style>
