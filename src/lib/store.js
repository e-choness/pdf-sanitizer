import { writable, derived } from 'svelte/store';

export const files = writable([]);

export const settings = writable({
  removeMetadata: true,
  removeScripts: true,
  removeEmbeddedFiles: true,
  compressImages: false,
  imageQuality: 'medium',
  stripExternalLinks: false,
  fontSubsetting: false,
  maxConcurrent: 4,
  outputFolder: '',
});

export const batchRunning = writable(false);
export const dragActive = writable(null);

export function formatSize(bytes) {
  if (!bytes || bytes <= 0) return '0 KB';
  if (bytes >= 1048576) {
    return (bytes / 1048576).toFixed(2) + ' MB';
  }
  return Math.round(bytes / 1024) + ' KB';
}

const SANITIZING_STAGES = new Set(['loading', 'rewriting', 'optimizing', 'saving', 'replacing']);

export function isActive(status) {
  return SANITIZING_STAGES.has(status) || status === 'verifying';
}

export function isStoppable(status) {
  return SANITIZING_STAGES.has(status);
}

export function clearFinished(fs) {
  return fs.filter(f => !['done', 'error', 'cancelled'].includes(f.status));
}

export function toggleSelectAll(fs, checked) {
  return fs.map(f => ({ ...f, selected: checked }));
}

export const headerSummary = derived([files, batchRunning], ([$files, $batchRunning]) => {
  if ($files.length === 0) return '';

  const sanitized = $files.filter(f => f.status === 'done').length;
  const sanitizing = $files.filter(f => SANITIZING_STAGES.has(f.status)).length;
  const verifying = $files.filter(f => f.status === 'verifying').length;
  const failed = $files.filter(f => f.status === 'error').length;
  const queued = $files.filter(f => f.status === 'pending').length;

  const isBatchDone = !$batchRunning && sanitizing === 0 && verifying === 0 && queued === 0;

  if (isBatchDone && (sanitized > 0 || failed > 0)) {
    const bytesSaved = $files
      .filter(f => f.status === 'done' && f.size && f.outputSize != null)
      .reduce((sum, f) => sum + (f.size - f.outputSize), 0);
    const parts = [];
    if (sanitized > 0) parts.push(`${sanitized} sanitized`);
    if (failed > 0) parts.push(`${failed} failed`);
    if (bytesSaved > 0) parts.push(`saved ${formatSize(bytesSaved)}`);
    return parts.join(' · ');
  }

  const parts = [];
  if (sanitized > 0) parts.push(`${sanitized} sanitized`);
  if (sanitizing > 0) parts.push(`${sanitizing} sanitizing`);
  if (verifying > 0) parts.push(`${verifying} verifying`);
  if (failed > 0) parts.push(`${failed} failed`);
  if (queued > 0) parts.push(`${queued} queued`);
  return parts.join(' · ');
});
