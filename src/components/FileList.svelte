<script>
  import { files, settings, batchRunning, headerSummary, clearFinished, toggleSelectAll, isActive } from '../lib/store.js';
  import FileRow from './FileRow.svelte';

  export let addFiles;
  export let startProcessing;
  export let cancelAll;
  export let cancelFile;
  export let selectFolder;

  $: pendingSelected = $files.filter(f => f.selected && f.status === 'pending').length;
  $: hasFinished = $files.some(f => ['done', 'error', 'cancelled'].includes(f.status));
  $: failedCount = $files.filter(f => f.status === 'error').length;
  $: anyActive = $files.some(f => isActive(f.status));
  $: allSelected = $files.length > 0 && $files.every(f => f.selected);
  $: someSelected = $files.some(f => f.selected) && !allSelected;

  function handleSelectAll(e) {
    files.set(toggleSelectAll($files, e.target.checked));
  }

  function removeFile(id) {
    files.update(fs => fs.filter(f => f.id !== id));
  }

  function retryFile(id) {
    files.update(fs => fs.map(f =>
      f.id === id
        ? { ...f, status: 'pending', error: null, report: null, outputSize: null, pct: 0, pages: null }
        : f
    ));
  }

  function retryFailed() {
    files.update(fs => fs.map(f =>
      f.status === 'error'
        ? { ...f, status: 'pending', error: null, report: null, outputSize: null, pct: 0, pages: null }
        : f
    ));
  }

  function doClearFinished() {
    files.update(clearFinished);
  }
</script>

<div class="panel">
  <!-- Header -->
  <header class="header">
    <span class="app-title">PDF Sanitizer</span>
    {#if $headerSummary}
      <span class="summary">{$headerSummary}</span>
    {/if}
  </header>

  <!-- Toolbar -->
  {#if $files.length > 0}
    <div class="toolbar">
      <label class="select-all">
        <input
          type="checkbox"
          checked={allSelected}
          indeterminate={someSelected}
          on:change={handleSelectAll}
        />
        <span class="count">{$files.length} {$files.length === 1 ? 'file' : 'files'}</span>
      </label>

      <div class="toolbar-actions">
        {#if hasFinished && !$batchRunning}
          <button class="btn-ghost" on:click={doClearFinished}>Clear finished</button>
        {/if}
        {#if failedCount > 0 && !$batchRunning}
          <button class="btn-ghost" on:click={retryFailed}>
            Retry {failedCount} failed
          </button>
        {/if}
        {#if anyActive}
          <button class="btn-ghost danger" on:click={cancelAll}>Stop all</button>
        {/if}
        <button class="btn-ghost" on:click={addFiles}>Add files…</button>
      </div>
    </div>
  {/if}

  <!-- List body -->
  <div class="list-body">
    {#if $files.length === 0}
      <div class="empty">
        <div class="empty-icon">
          <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
            <polyline points="14 2 14 8 20 8"/>
          </svg>
        </div>
        <div class="empty-title">No PDFs added</div>
        <div class="empty-sub">Drop files here or click Add files to get started</div>
        <button class="btn-primary" on:click={addFiles}>Add files…</button>
      </div>
    {:else}
      {#each $files as file (file.id)}
        <FileRow
          {file}
          {cancelFile}
          on:remove={e => removeFile(e.detail)}
          on:retry={e => retryFile(e.detail)}
          on:toggle={e => files.update(fs => fs.map(f => f.id === e.detail ? { ...f, selected: !f.selected } : f))}
        />
      {/each}
      <div class="drop-hint">Drop more PDFs here</div>
    {/if}
  </div>

  <!-- Footer -->
  <footer class="footer">
    <button class="folder-btn" on:click={selectFolder} title={$settings.outputFolder || 'Choose backup folder'}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
      </svg>
      <span class="folder-label">
        {#if $settings.outputFolder}
          {$settings.outputFolder.split(/[\\/]/).pop() || $settings.outputFolder}
        {:else}
          <span class="folder-empty">Choose backup folder</span>
        {/if}
      </span>
    </button>

    <div class="footer-right">
      {#if pendingSelected > 0}
        <span class="selected-count">{pendingSelected} selected</span>
      {/if}
      <button
        class="btn-primary"
        disabled={pendingSelected === 0 || !$settings.outputFolder || $batchRunning}
        on:click={startProcessing}
      >
        {#if $batchRunning}
          Sanitizing…
        {:else}
          Sanitize {pendingSelected === 1 ? '1 file' : `${pendingSelected} files`}
        {/if}
      </button>
    </div>
  </footer>
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--surface);
    border-right: 1px solid var(--border);
    overflow: hidden;
  }

  /* Header */
  .header {
    height: 44px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 16px;
    border-bottom: 1px solid var(--border);
  }

  .app-title {
    font-size: 13px;
    font-weight: 600;
    flex-shrink: 0;
  }

  .summary {
    font-size: 12px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Toolbar */
  .toolbar {
    height: 40px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 16px;
    border-bottom: 1px solid var(--border);
  }

  .select-all {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    user-select: none;
  }

  .select-all input[type="checkbox"] {
    width: 14px;
    height: 14px;
    cursor: pointer;
    accent-color: var(--accent);
    flex-shrink: 0;
  }

  .count {
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
  }

  .toolbar-actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  /* List body */
  .list-body {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    height: 100%;
    padding: 32px;
    text-align: center;
  }

  .empty-icon {
    color: var(--border2);
  }

  .empty-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--muted);
  }

  .empty-sub {
    font-size: 12px;
    color: var(--muted);
    max-width: 240px;
  }

  .drop-hint {
    padding: 12px 16px;
    font-size: 11px;
    color: var(--border2);
    text-align: center;
  }

  /* Footer */
  .footer {
    height: 56px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 16px;
    border-top: 1px solid var(--border);
  }

  .folder-btn {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 8px;
    cursor: pointer;
    color: var(--text);
    font-size: 12px;
    min-width: 0;
    height: 30px;
  }

  .folder-btn:hover { background: var(--surface2); }

  .folder-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .folder-empty { color: var(--muted); }

  .footer-right {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .selected-count {
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
  }

  /* Buttons */
  .btn-primary {
    height: 30px;
    padding: 0 14px;
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    font-family: inherit;
  }

  .btn-primary:hover:not(:disabled) { background: var(--accent-h); }
  .btn-primary:disabled { opacity: 0.45; cursor: not-allowed; }

  .btn-ghost {
    height: 26px;
    padding: 0 9px;
    background: none;
    border: 1px solid var(--border);
    border-radius: 5px;
    font-size: 12px;
    color: var(--text);
    cursor: pointer;
    font-family: inherit;
    white-space: nowrap;
  }

  .btn-ghost:hover { background: var(--surface2); }
  .btn-ghost.danger { color: var(--err); border-color: var(--errbg); }
  .btn-ghost.danger:hover { background: var(--errbg); }
</style>
