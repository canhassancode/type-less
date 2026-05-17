import { describe, expect, test } from 'vitest';
import { initialState, type ManagerState, type ModelRowState, reduce } from './downloadManager';

const NOT_INSTALLED: ModelRowState = { status: 'not-installed' };
const INSTALLED: ModelRowState = { status: 'installed' };

function withRow(id: string, row: ModelRowState): ManagerState {
  return { rows: { [id]: row } };
}

function row(state: ManagerState, id: string): ModelRowState {
  const found = state.rows[id];
  if (!found) throw new Error(`expected row for ${id}`);
  return found;
}

describe('downloadManager.reduce', () => {
  test('initial state has no rows', () => {
    expect(initialState).toEqual({ rows: {} });
  });

  test('installation-checked installed populates an installed row', () => {
    const next = reduce(initialState, {
      type: 'installation-checked',
      modelId: 'whisper',
      status: 'installed',
    });

    expect(next).toEqual(withRow('whisper', INSTALLED));
  });

  test('installation-checked not_installed populates a not-installed row', () => {
    const next = reduce(initialState, {
      type: 'installation-checked',
      modelId: 'whisper',
      status: 'not_installed',
    });

    expect(next).toEqual(withRow('whisper', NOT_INSTALLED));
  });

  test('installation-checked checksum_mismatch surfaces as failed with reason', () => {
    const next = reduce(initialState, {
      type: 'installation-checked',
      modelId: 'whisper',
      status: 'checksum_mismatch',
    });

    expect(row(next, 'whisper')).toEqual({
      status: 'failed',
      message: 'checksum mismatch on disk',
    });
  });

  test('download-clicked on not-installed transitions to downloading at 0/0', () => {
    const state = withRow('whisper', NOT_INSTALLED);

    const next = reduce(state, { type: 'download-clicked', modelId: 'whisper' });

    expect(row(next, 'whisper')).toEqual({
      status: 'downloading',
      bytesDownloaded: 0,
      bytesTotal: 0,
    });
  });

  test('download-clicked on failed row retries (transitions to downloading)', () => {
    const state = withRow('whisper', { status: 'failed', message: 'network drop' });

    const next = reduce(state, { type: 'download-clicked', modelId: 'whisper' });

    expect(row(next, 'whisper').status).toBe('downloading');
  });

  test('download-clicked on installed row triggers re-download', () => {
    const state = withRow('whisper', INSTALLED);

    const next = reduce(state, { type: 'download-clicked', modelId: 'whisper' });

    expect(row(next, 'whisper').status).toBe('downloading');
  });

  test('progress while downloading updates byte counts', () => {
    const state = withRow('whisper', {
      status: 'downloading',
      bytesDownloaded: 0,
      bytesTotal: 0,
    });

    const next = reduce(state, {
      type: 'progress',
      modelId: 'whisper',
      bytesDownloaded: 1024,
      bytesTotal: 8192,
    });

    expect(row(next, 'whisper')).toEqual({
      status: 'downloading',
      bytesDownloaded: 1024,
      bytesTotal: 8192,
    });
  });

  test('completed while downloading transitions to installed', () => {
    const state = withRow('whisper', {
      status: 'downloading',
      bytesDownloaded: 8192,
      bytesTotal: 8192,
    });

    const next = reduce(state, { type: 'completed', modelId: 'whisper' });

    expect(row(next, 'whisper')).toEqual(INSTALLED);
  });

  test('failed while downloading transitions to failed with message', () => {
    const state = withRow('whisper', {
      status: 'downloading',
      bytesDownloaded: 100,
      bytesTotal: 8192,
    });

    const next = reduce(state, {
      type: 'failed',
      modelId: 'whisper',
      message: 'checksum mismatch',
    });

    expect(row(next, 'whisper')).toEqual({
      status: 'failed',
      message: 'checksum mismatch',
    });
  });

  test('download-clicked while already downloading is a no-op', () => {
    const downloading: ModelRowState = {
      status: 'downloading',
      bytesDownloaded: 2048,
      bytesTotal: 8192,
    };
    const state = withRow('whisper', downloading);

    const next = reduce(state, { type: 'download-clicked', modelId: 'whisper' });

    expect(next).toBe(state);
  });

  test('events for unknown model ids leave state unchanged', () => {
    const state = withRow('whisper', INSTALLED);

    const next = reduce(state, {
      type: 'progress',
      modelId: 'ghost',
      bytesDownloaded: 1,
      bytesTotal: 1,
    });

    expect(next).toBe(state);
  });
});
