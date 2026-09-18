import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  files,
  batchRunning,
  headerSummary,
  formatSize,
  clearFinished,
  toggleSelectAll,
} from './store.js';

function makeFile(id, status, size = 1000, outputSize = null, selected = true) {
  return { id, status, size, outputSize, selected };
}

beforeEach(() => {
  files.set([]);
  batchRunning.set(false);
});

describe('formatSize', () => {
  it('formats bytes >= 1 MB with 2 decimal places', () => {
    expect(formatSize(1_400_000)).toBe('1.34 MB');
  });
  it('formats bytes < 1 MB as KB with 0 decimal places', () => {
    expect(formatSize(100 * 1024)).toBe('100 KB');
  });
  it('rounds KB', () => {
    expect(formatSize(500_000)).toBe('488 KB');
  });
  it('returns 0 KB for zero', () => {
    expect(formatSize(0)).toBe('0 KB');
  });
  it('returns 0 KB for null/undefined', () => {
    expect(formatSize(null)).toBe('0 KB');
  });
});

describe('headerSummary', () => {
  it('returns empty string when no files', () => {
    expect(get(headerSummary)).toBe('');
  });

  it('shows live counts during a batch', () => {
    batchRunning.set(true);
    files.set([
      makeFile('1', 'done'),
      makeFile('2', 'rewriting'),
      makeFile('3', 'verifying'),
      makeFile('4', 'error'),
      makeFile('5', 'pending'),
    ]);
    expect(get(headerSummary)).toBe(
      '1 sanitized · 1 sanitizing · 1 verifying · 1 failed · 1 queued'
    );
  });

  it('shows post-batch summary after batch finishes', () => {
    batchRunning.set(false);
    files.set([
      makeFile('1', 'done', 2_000_000, 1_000_000),
      makeFile('2', 'done', 2_000_000, 1_500_000),
      makeFile('3', 'error'),
    ]);
    const s = get(headerSummary);
    expect(s).toContain('2 sanitized');
    expect(s).toContain('1 failed');
    expect(s).toContain('saved');
  });

  it('omits zero-count groups', () => {
    batchRunning.set(true);
    files.set([makeFile('1', 'rewriting'), makeFile('2', 'pending')]);
    expect(get(headerSummary)).toBe('1 sanitizing · 1 queued');
  });
});

describe('clearFinished', () => {
  it('removes done, error, and cancelled files', () => {
    const input = [
      makeFile('1', 'done'),
      makeFile('2', 'error'),
      makeFile('3', 'cancelled'),
      makeFile('4', 'pending'),
      makeFile('5', 'rewriting'),
    ];
    const result = clearFinished(input);
    expect(result).toHaveLength(2);
    expect(result.map(f => f.id)).toEqual(['4', '5']);
  });

  it('returns all files when none are finished', () => {
    const input = [makeFile('1', 'pending'), makeFile('2', 'rewriting')];
    expect(clearFinished(input)).toHaveLength(2);
  });
});

describe('toggleSelectAll', () => {
  it('sets all files to selected=true', () => {
    const input = [makeFile('1', 'pending', 0, null, false), makeFile('2', 'done', 0, null, false)];
    expect(toggleSelectAll(input, true).every(f => f.selected)).toBe(true);
  });

  it('sets all files to selected=false', () => {
    const input = [makeFile('1', 'pending', 0, null, true), makeFile('2', 'done', 0, null, true)];
    expect(toggleSelectAll(input, false).every(f => f.selected)).toBe(false);
  });
});
