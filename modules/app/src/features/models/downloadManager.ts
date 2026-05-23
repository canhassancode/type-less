import type { ModelStatus } from '../../shared/ipc/bindings';

export type ModelRowState =
  | { status: 'not-installed' }
  | { status: 'downloading'; bytesDownloaded: number; bytesTotal: number }
  | { status: 'installed' }
  | { status: 'failed'; message: string };

export interface ManagerState {
  rows: Record<string, ModelRowState>;
}

export type ManagerEvent =
  | { type: 'installation-checked'; modelId: string; status: ModelStatus }
  | { type: 'download-clicked'; modelId: string }
  | { type: 'progress'; modelId: string; bytesDownloaded: number; bytesTotal: number }
  | { type: 'completed'; modelId: string }
  | { type: 'failed'; modelId: string; message: string };

export const initialState: ManagerState = { rows: {} };

const FRESH_DOWNLOAD: ModelRowState = {
  status: 'downloading',
  bytesDownloaded: 0,
  bytesTotal: 0,
};

const CHECKSUM_FAILURE: ModelRowState = {
  status: 'failed',
  message: 'checksum mismatch on disk',
};

export function reduce(state: ManagerState, event: ManagerEvent): ManagerState {
  switch (event.type) {
    case 'installation-checked':
      return setRow(state, event.modelId, fromInstallationStatus(event.status));

    case 'download-clicked': {
      const current = state.rows[event.modelId];
      if (current?.status === 'downloading') return state;
      return setRow(state, event.modelId, FRESH_DOWNLOAD);
    }

    case 'progress':
      return updateExisting(state, event.modelId, (row) =>
        row.status === 'downloading'
          ? {
              status: 'downloading',
              bytesDownloaded: event.bytesDownloaded,
              bytesTotal: event.bytesTotal,
            }
          : row,
      );

    case 'completed':
      return updateExisting(state, event.modelId, () => ({ status: 'installed' }));

    case 'failed':
      return updateExisting(state, event.modelId, () => ({
        status: 'failed',
        message: event.message,
      }));
  }
}

function setRow(state: ManagerState, modelId: string, row: ModelRowState): ManagerState {
  return { rows: { ...state.rows, [modelId]: row } };
}

function updateExisting(
  state: ManagerState,
  modelId: string,
  update: (row: ModelRowState) => ModelRowState,
): ManagerState {
  const current = state.rows[modelId];
  if (!current) return state;
  const next = update(current);
  if (next === current) return state;
  return setRow(state, modelId, next);
}

function fromInstallationStatus(status: ModelStatus): ModelRowState {
  switch (status) {
    case 'installed':
      return { status: 'installed' };
    case 'not_installed':
      return { status: 'not-installed' };
    case 'checksum_mismatch':
      return CHECKSUM_FAILURE;
  }
}
