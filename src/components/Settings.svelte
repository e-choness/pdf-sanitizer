<script>
  import { invoke } from '@tauri-apps/api/core';
  import { settings, batchRunning } from '../lib/store.js';

  export let selectFolder;

  let saveTimer;

  function queueSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      invoke('save_settings', { newSettings: $settings }).catch(console.error);
    }, 300);
  }

  function setQuality(q) {
    $settings = { ...$settings, imageQuality: q };
    queueSave();
  }

  function decConcurrent() {
    if ($settings.maxConcurrent <= 1) return;
    $settings = { ...$settings, maxConcurrent: $settings.maxConcurrent - 1 };
    queueSave();
  }

  function incConcurrent() {
    if ($settings.maxConcurrent >= 8) return;
    $settings = { ...$settings, maxConcurrent: $settings.maxConcurrent + 1 };
    queueSave();
  }
</script>

<aside class="panel">
  <div class="panel-header">Settings</div>

  <div class="section">
    <div class="section-label">Content</div>

    <label class="row" class:disabled={$batchRunning}>
      <div class="row-text">
        <span class="row-title">Remove metadata</span>
        <span class="row-sub">Author, dates, software</span>
      </div>
      <button
        role="switch"
        aria-checked={$settings.removeMetadata}
        aria-label="Remove metadata"
        class="switch"
        class:on={$settings.removeMetadata}
        disabled={$batchRunning}
        on:click={() => { $settings = { ...$settings, removeMetadata: !$settings.removeMetadata }; queueSave(); }}
      ></button>
    </label>

    <label class="row" class:disabled={$batchRunning}>
      <div class="row-text">
        <span class="row-title">Remove scripts</span>
        <span class="row-sub">JavaScript and actions</span>
      </div>
      <button
        role="switch"
        aria-checked={$settings.removeScripts}
        aria-label="Remove scripts"
        class="switch"
        class:on={$settings.removeScripts}
        disabled={$batchRunning}
        on:click={() => { $settings = { ...$settings, removeScripts: !$settings.removeScripts }; queueSave(); }}
      ></button>
    </label>

    <label class="row" class:disabled={$batchRunning}>
      <div class="row-text">
        <span class="row-title">Remove embedded files</span>
        <span class="row-sub">Attachments and streams</span>
      </div>
      <button
        role="switch"
        aria-checked={$settings.removeEmbeddedFiles}
        aria-label="Remove embedded files"
        class="switch"
        class:on={$settings.removeEmbeddedFiles}
        disabled={$batchRunning}
        on:click={() => { $settings = { ...$settings, removeEmbeddedFiles: !$settings.removeEmbeddedFiles }; queueSave(); }}
      ></button>
    </label>

    <label class="row" class:disabled={$batchRunning}>
      <div class="row-text">
        <span class="row-title">Strip external links</span>
        <span class="row-sub">Remove outbound URLs</span>
      </div>
      <button
        role="switch"
        aria-checked={$settings.stripExternalLinks}
        aria-label="Strip external links"
        class="switch"
        class:on={$settings.stripExternalLinks}
        disabled={$batchRunning}
        on:click={() => { $settings = { ...$settings, stripExternalLinks: !$settings.stripExternalLinks }; queueSave(); }}
      ></button>
    </label>

    <label class="row" class:disabled={$batchRunning}>
      <div class="row-text">
        <span class="row-title">Font subsetting</span>
        <span class="row-sub">Keep only used glyphs</span>
      </div>
      <button
        role="switch"
        aria-checked={$settings.fontSubsetting}
        aria-label="Font subsetting"
        class="switch"
        class:on={$settings.fontSubsetting}
        disabled={$batchRunning}
        on:click={() => { $settings = { ...$settings, fontSubsetting: !$settings.fontSubsetting }; queueSave(); }}
      ></button>
    </label>
  </div>

  <div class="section">
    <div class="section-label">Images</div>

    <label class="row" class:disabled={$batchRunning}>
      <div class="row-text">
        <span class="row-title">Compress images</span>
        <span class="row-sub">Re-encode to JPEG</span>
      </div>
      <button
        role="switch"
        aria-checked={$settings.compressImages}
        aria-label="Compress images"
        class="switch"
        class:on={$settings.compressImages}
        disabled={$batchRunning}
        on:click={() => { $settings = { ...$settings, compressImages: !$settings.compressImages }; queueSave(); }}
      ></button>
    </label>

    {#if $settings.compressImages}
      <div class="quality-row">
        <span class="row-sub">Quality</span>
        <div class="segmented">
          {#each ['low', 'medium', 'high'] as q}
            <button
              class="seg-btn"
              class:active={$settings.imageQuality === q}
              disabled={$batchRunning}
              on:click={() => setQuality(q)}
            >
              {q[0].toUpperCase() + q.slice(1)}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  <div class="section">
    <div class="section-label">Performance</div>

    <div class="row">
      <div class="row-text">
        <span class="row-title">Concurrent files</span>
        <span class="row-sub">Files processed in parallel</span>
      </div>
      <div class="stepper">
        <button class="step-btn" on:click={decConcurrent} disabled={$settings.maxConcurrent <= 1 || $batchRunning}>−</button>
        <span class="step-val">{$settings.maxConcurrent}</span>
        <button class="step-btn" on:click={incConcurrent} disabled={$settings.maxConcurrent >= 8 || $batchRunning}>+</button>
      </div>
    </div>
  </div>

  <div class="section">
    <div class="section-label">Output</div>

    <div class="folder-row">
      <div class="row-text">
        <span class="row-title">Backup folder</span>
        <span class="row-sub">Originals are moved here</span>
      </div>
      <button class="folder-pick-btn" on:click={selectFolder} disabled={$batchRunning}>
        {#if $settings.outputFolder}
          {$settings.outputFolder.split(/[\\/]/).pop() || $settings.outputFolder}
        {:else}
          Choose…
        {/if}
      </button>
    </div>
    {#if !$settings.outputFolder}
      <p class="folder-warn">A backup folder is required to sanitize files.</p>
    {/if}
  </div>
</aside>

<style>
  .panel {
    height: 100vh;
    overflow-y: auto;
    background: var(--canvas);
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
  }

  .panel-header {
    height: 44px;
    display: flex;
    align-items: center;
    padding: 0 16px;
    font-size: 13px;
    font-weight: 600;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .section {
    padding: 12px 0;
    border-bottom: 1px solid var(--border);
  }

  .section-label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
    padding: 0 16px 6px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 16px;
    cursor: default;
  }

  .row.disabled { opacity: 0.5; pointer-events: none; }

  .folder-row {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 7px 16px;
    flex-direction: column;
  }

  .row-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .row-title {
    font-size: 13px;
  }

  .row-sub {
    font-size: 11px;
    color: var(--muted);
  }

  /* Toggle switch */
  .switch {
    position: relative;
    width: 32px;
    height: 18px;
    border-radius: 9px;
    background: var(--track);
    border: none;
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.15s;
    padding: 0;
  }

  .switch::after {
    content: '';
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--knob);
    box-shadow: 0 1px 3px rgba(0,0,0,.25);
    transition: transform 0.15s;
  }

  .switch.on {
    background: var(--accent);
  }

  .switch.on::after {
    transform: translateX(14px);
  }

  .switch:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  /* Quality segmented control */
  .quality-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 16px 4px;
  }

  .segmented {
    display: flex;
    border: 1px solid var(--border2);
    border-radius: 6px;
    overflow: hidden;
  }

  .seg-btn {
    padding: 3px 10px;
    background: none;
    border: none;
    border-right: 1px solid var(--border2);
    font-size: 11px;
    font-family: inherit;
    color: var(--muted);
    cursor: pointer;
  }

  .seg-btn:last-child { border-right: none; }
  .seg-btn:hover:not(.active):not(:disabled) { background: var(--surface2); }
  .seg-btn.active { background: var(--accent); color: #fff; }
  .seg-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  /* Stepper */
  .stepper {
    display: flex;
    align-items: center;
    gap: 0;
    border: 1px solid var(--border2);
    border-radius: 6px;
    overflow: hidden;
    height: 26px;
  }

  .step-btn {
    width: 26px;
    height: 26px;
    background: none;
    border: none;
    border-right: 1px solid var(--border2);
    font-size: 14px;
    cursor: pointer;
    color: var(--text);
    font-family: inherit;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .step-btn:last-child { border-right: none; border-left: 1px solid var(--border2); }
  .step-btn:hover:not(:disabled) { background: var(--surface2); }
  .step-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .step-val {
    width: 26px;
    text-align: center;
    font-size: 13px;
    font-weight: 500;
  }

  /* Folder */
  .folder-pick-btn {
    width: 100%;
    padding: 5px 10px;
    background: var(--surface);
    border: 1px solid var(--border2);
    border-radius: 6px;
    font-size: 12px;
    font-family: inherit;
    color: var(--text);
    cursor: pointer;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .folder-pick-btn:hover:not(:disabled) { background: var(--surface2); }
  .folder-pick-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .folder-warn {
    font-size: 11px;
    color: var(--err);
    padding: 2px 16px 6px;
  }
</style>
