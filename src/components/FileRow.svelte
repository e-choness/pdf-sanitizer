<script>
  import { createEventDispatcher } from 'svelte';
  import { formatSize, isActive, isStoppable } from '../lib/store.js';

  export let file;
  export let cancelFile;

  const dispatch = createEventDispatcher();

  function stageText(f) {
    switch (f.status) {
      case 'pending':    return f.folder;
      case 'loading':    return 'Opening and parsing document';
      case 'rewriting':  return `Rewriting objects · ${f.pct}%`;
      case 'optimizing': return 'Optimizing';
      case 'saving':     return 'Writing sanitized copy';
      case 'verifying':  return f.pages ? `Checking all ${f.pages} pages` : 'Verifying result';
      case 'replacing':  return 'Finalizing';
      case 'done':       return reportSummary(f.report);
      case 'error':      return f.error ?? 'Failed';
      case 'cancelled':  return 'Stopped';
      default:           return '';
    }
  }

  function reportSummary(report) {
    if (!report) return 'Rewritten and verified';
    const parts = [];
    if (report.removed_metadata) parts.push('Removed metadata');
    if (report.removed_javascript > 0) {
      parts.push(`${report.removed_javascript} script${report.removed_javascript > 1 ? 's' : ''}`);
    }
    if (report.removed_actions > 0) {
      parts.push(`${report.removed_actions} action${report.removed_actions > 1 ? 's' : ''}`);
    }
    if (report.removed_embedded_files > 0) {
      parts.push(`${report.removed_embedded_files} attachment${report.removed_embedded_files > 1 ? 's' : ''}`);
    }
    if (report.removed_links > 0) {
      parts.push(`${report.removed_links} link${report.removed_links > 1 ? 's' : ''}`);
    }
    if (report.images_recompressed > 0) {
      parts.push(`${report.images_recompressed} image${report.images_recompressed > 1 ? 's' : ''} compressed`);
    }
    if (parts.length === 0) return 'Nothing to remove · rewritten and verified';
    return parts.join(' · ');
  }

  function pillClass(status) {
    if (status === 'done') return 'pill ok';
    if (status === 'error') return 'pill err';
    if (status === 'cancelled') return 'pill stopped';
    if (status === 'pending') return 'pill queued';
    return 'pill proc';
  }

  function pillLabel(status) {
    if (status === 'done') return 'Sanitized';
    if (status === 'error') return 'Failed';
    if (status === 'cancelled') return 'Stopped';
    if (status === 'pending') return 'Queued';
    if (status === 'verifying') return 'Verifying';
    return 'Sanitizing';
  }

  function handleAction() {
    if (isStoppable(file.status)) {
      cancelFile(file.id);
    } else if (file.status === 'error') {
      dispatch('retry', file.id);
    } else if (!isActive(file.status)) {
      dispatch('remove', file.id);
    }
  }

  function actionTitle(status) {
    if (isStoppable(status)) return 'Stop';
    if (status === 'error') return 'Retry';
    if (!isActive(status)) return 'Remove';
    return '';
  }

  $: showAction = isStoppable(file.status) || file.status === 'error' || !isActive(file.status);
  $: active = isActive(file.status);
  $: stoppable = isStoppable(file.status);
  $: sub = stageText(file);

  $: outputBigger = file.outputSize != null && file.size > 0 && file.outputSize > file.size;
</script>

<div class="row" class:active>
  <!-- Col 1: Checkbox -->
  <label class="check-wrap">
    <input
      type="checkbox"
      checked={file.selected}
      disabled={active}
      on:change={() => dispatch('toggle', file.id)}
    />
  </label>

  <!-- Col 2: Name + subtext -->
  <div class="info">
    <span class="name" title={file.path}>{file.name}</span>
    <span class="sub" class:err={file.status === 'error'} title={sub}>{sub}</span>
  </div>

  <!-- Col 3: Sizes -->
  <div class="sizes">
    {#if file.status === 'done' && file.outputSize != null}
      <span class="size-in">{formatSize(file.size)}</span>
      <span class="arrow">→</span>
      <span class="size-out" class:bigger={outputBigger}>{formatSize(file.outputSize)}</span>
    {:else}
      <span class="size-in">{formatSize(file.size)}</span>
    {/if}
  </div>

  <!-- Col 4: Status pill -->
  <div class="pill-wrap">
    <span class={pillClass(file.status)}>
      {#if file.status === 'done'}
        <span class="dot"></span>
      {:else if active}
        <span class="spinner"></span>
      {/if}
      {pillLabel(file.status)}
    </span>
  </div>

  <!-- Col 5: Action button -->
  <div class="action-wrap">
    {#if showAction}
      <button
        class="action-btn"
        class:stop={stoppable}
        class:retry={file.status === 'error'}
        title={actionTitle(file.status)}
        on:click={handleAction}
      >
        {#if stoppable}
          ■
        {:else if file.status === 'error'}
          ↺
        {:else}
          ✕
        {/if}
      </button>
    {/if}
  </div>

  <!-- Progress bar -->
  {#if active}
    <div class="progress-track">
      <div class="progress-fill" style="width: {file.pct}%"></div>
    </div>
  {/if}
</div>

<style>
  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .row {
    position: relative;
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr) 150px 120px 28px;
    grid-template-rows: 1fr;
    column-gap: 14px;
    align-items: center;
    height: 52px;
    padding: 0 16px;
    border-bottom: 1px solid var(--border);
  }

  .row:hover { background: var(--surface2); }

  @media (max-width: 900px) {
    .row {
      grid-template-columns: 14px minmax(0, 1fr) 128px 92px 24px;
      column-gap: 10px;
      height: 46px;
    }
  }

  /* Checkbox */
  .check-wrap {
    display: flex;
    align-items: center;
    cursor: pointer;
  }

  .check-wrap input[type="checkbox"] {
    width: 14px;
    height: 14px;
    cursor: pointer;
    accent-color: var(--accent);
  }

  .check-wrap input:disabled { cursor: default; opacity: 0.4; }

  /* Info */
  .info {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .name {
    font-size: 13px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    font-size: 11px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub.err { color: var(--err); }

  /* Sizes */
  .sizes {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    justify-content: flex-end;
    color: var(--muted);
    white-space: nowrap;
  }

  .size-in { color: var(--text); }
  .size-out { color: var(--ok); }
  .size-out.bigger { color: var(--err); }
  .arrow { color: var(--muted); font-size: 10px; }

  /* Pill */
  .pill-wrap {
    display: flex;
    justify-content: flex-end;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 500;
    padding: 3px 8px;
    border-radius: 100px;
    white-space: nowrap;
  }

  .pill.ok      { background: var(--okbg);   color: var(--ok);   }
  .pill.err     { background: var(--errbg);  color: var(--err);  }
  .pill.proc    { background: var(--procbg); color: var(--accent); }
  .pill.stopped { background: var(--surface2); color: var(--muted); }
  .pill.queued  { background: none; color: var(--muted); padding-inline: 4px; }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--ok);
    flex-shrink: 0;
  }

  .spinner {
    width: 10px;
    height: 10px;
    border: 1.5px solid transparent;
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.75s linear infinite;
    flex-shrink: 0;
  }

  /* Action */
  .action-wrap {
    display: flex;
    justify-content: center;
    align-items: center;
  }

  .action-btn {
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    cursor: pointer;
    font-size: 10px;
    color: var(--muted);
    padding: 0;
    line-height: 1;
  }

  .action-btn:hover { background: var(--surface2); color: var(--text); }
  .action-btn.stop:hover { background: var(--errbg); color: var(--err); border-color: var(--errbg); }
  .action-btn.retry { color: var(--accent); }

  /* Progress */
  .progress-track {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--border);
    grid-column: 1 / -1;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s ease;
  }
</style>
