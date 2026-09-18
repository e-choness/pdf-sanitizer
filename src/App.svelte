<script>
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { files, settings, batchRunning, dragActive } from './lib/store.js';
  import FileList from './components/FileList.svelte';
  import Settings from './components/Settings.svelte';
  import './app.css';

  let toasts = [];

  onMount(async () => {
    try {
      const saved = await invoke('load_settings');
      settings.update(s => ({ ...s, ...saved }));
    } catch (e) {
      console.error('Failed to load settings:', e);
    }

    const unlisteners = await Promise.all([
      listen('file_progress', ({ payload }) => {
        files.update(fs => fs.map(f =>
          f.id === payload.id
            ? { ...f, status: payload.stage, pct: payload.pct, pages: payload.pages ?? f.pages }
            : f
        ));
      }),

      listen('file_complete', ({ payload }) => {
        files.update(fs => fs.map(f =>
          f.id === payload.id
            ? { ...f, status: 'done', outputSize: payload.output_size, report: payload.report, backupPath: payload.backup_path }
            : f
        ));
      }),

      listen('file_error', ({ payload }) => {
        files.update(fs => fs.map(f =>
          f.id === payload.id
            ? { ...f, status: 'error', error: payload.error }
            : f
        ));
      }),

      listen('file_cancelled', ({ payload }) => {
        files.update(fs => fs.map(f =>
          f.id === payload.id ? { ...f, status: 'cancelled' } : f
        ));
      }),

      listen('batch_complete', () => {
        batchRunning.set(false);
      }),

      listen('tauri://drag-enter', ({ payload }) => {
        const count = payload?.paths?.filter(p => p.toLowerCase().endsWith('.pdf')).length ?? null;
        dragActive.set({ count });
      }),

      listen('tauri://drag-over', () => {
        dragActive.update(d => d ?? { count: null });
      }),

      listen('tauri://drag-leave', () => {
        dragActive.set(null);
      }),

      listen('tauri://drag-drop', ({ payload }) => {
        dragActive.set(null);
        if (payload?.paths?.length) addFilePaths(payload.paths);
      }),
    ]);

    return () => unlisteners.forEach(u => u());
  });

  async function addFilePaths(paths) {
    const pdfPaths = paths.filter(p => p.toLowerCase().endsWith('.pdf'));
    const skipped = paths.length - pdfPaths.length;

    if (skipped > 0) {
      showToast(skipped === 1 ? '1 non-PDF file ignored' : `${skipped} non-PDF files ignored`);
    }
    if (pdfPaths.length === 0) return;

    const existing = new Set(get(files).map(f => f.path));
    const newPaths = pdfPaths.filter(p => !existing.has(p));
    if (newPaths.length === 0) return;

    let stats = [];
    try {
      stats = await invoke('stat_files', { paths: newPaths });
    } catch {
      stats = newPaths.map(p => ({ path: p, size: 0 }));
    }

    const newFiles = stats.map(({ path, size }) => ({
      id: crypto.randomUUID(),
      name: path.split(/[\\/]/).pop(),
      folder: path.replace(/[\\/][^\\/]+$/, ''),
      path,
      size,
      outputSize: null,
      status: 'pending',
      pct: 0,
      pages: null,
      error: null,
      report: null,
      backupPath: null,
      selected: true,
    }));

    files.update(fs => [...fs, ...newFiles]);
  }

  async function addFiles() {
    try {
      const selected = await open({
        multiple: true,
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      });
      if (!selected) return;
      addFilePaths(Array.isArray(selected) ? selected : [selected]);
    } catch (e) {
      console.error('open dialog failed:', e);
    }
  }

  async function selectFolder() {
    try {
      const folder = await open({ directory: true, multiple: false });
      if (!folder) return;
      settings.update(s => {
        const updated = { ...s, outputFolder: folder };
        invoke('save_settings', { newSettings: updated }).catch(console.error);
        return updated;
      });
    } catch (e) {
      console.error('folder dialog failed:', e);
    }
  }

  async function startProcessing() {
    const currentFiles = get(files);
    const currentSettings = get(settings);
    const selected = currentFiles.filter(f => f.selected && f.status === 'pending');
    if (selected.length === 0 || !currentSettings.outputFolder) return;

    try {
      batchRunning.set(true);
      await invoke('process_files', {
        files: selected.map(f => ({ id: f.id, path: f.path })),
      });
    } catch (e) {
      batchRunning.set(false);
      console.error('process_files failed:', e);
    }
  }

  async function cancelAll() {
    try { await invoke('cancel_all'); } catch (e) { console.error(e); }
  }

  async function cancelFile(id) {
    try { await invoke('cancel_file', { id }); } catch (e) { console.error(e); }
  }

  function showToast(message) {
    const id = Date.now();
    toasts = [...toasts, { id, message }];
    setTimeout(() => { toasts = toasts.filter(t => t.id !== id); }, 2600);
  }
</script>

<div class="app">
  <FileList {addFiles} {startProcessing} {cancelAll} {cancelFile} {selectFolder} />
  <Settings {selectFolder} />
</div>

{#if $dragActive}
  <div class="drag-overlay">
    <div class="drag-card">
      <div class="drag-title">
        {#if $dragActive.count !== null}
          Drop to add {$dragActive.count === 1 ? '1 PDF' : `${$dragActive.count} PDFs`}
        {:else}
          Drop to add PDFs
        {/if}
      </div>
      <div class="drag-sub">Non-PDF files are ignored</div>
    </div>
  </div>
{/if}

{#if toasts.length}
  <div class="toasts">
    {#each toasts as toast (toast.id)}
      <div class="toast">{toast.message}</div>
    {/each}
  </div>
{/if}

<style>
  .app {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    height: 100vh;
    overflow: hidden;
  }

  @media (max-width: 900px) {
    .app { grid-template-columns: minmax(0, 1fr) 248px; }
  }

  .drag-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.15);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    outline: 2px dashed var(--accent);
    outline-offset: -12px;
    pointer-events: none;
  }

  .drag-card {
    text-align: center;
    padding: 24px 32px;
    background: var(--surface);
    border-radius: 12px;
    border: 1.5px solid var(--accent);
  }

  .drag-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--accent);
    margin-bottom: 6px;
  }

  .drag-sub {
    font-size: 12px;
    color: var(--muted);
  }

  .toasts {
    position: fixed;
    bottom: 16px;
    left: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 200;
    pointer-events: none;
  }

  .toast {
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
    font-size: 12px;
    padding: 8px 12px;
    border-radius: 6px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.12);
  }
</style>
