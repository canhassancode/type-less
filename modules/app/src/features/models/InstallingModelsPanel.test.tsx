import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';
import type { Model } from '../../shared/ipc/bindings';
import type { ManagerState } from './downloadManager';
import { InstallingModelsPanel } from './InstallingModelsPanel';

const WHISPER: Model = {
  id: 'whisper',
  purpose: 'asr',
  url: 'https://huggingface.co/x',
  sha256: '0'.repeat(64),
  size_bytes: 488_000_000,
  filename: 'ggml-small.en.bin',
};

describe('InstallingModelsPanel', () => {
  test('shows the model row with a Download button when not installed', () => {
    const state: ManagerState = { rows: {} };
    const onDownload = vi.fn();

    render(<InstallingModelsPanel models={[WHISPER]} state={state} onDownload={onDownload} />);

    expect(screen.getByText(WHISPER.filename)).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Download' }));
    expect(onDownload).toHaveBeenCalledWith('whisper');
  });

  test('shows downloading percent when a download is in flight', () => {
    const state: ManagerState = {
      rows: {
        whisper: {
          status: 'downloading',
          bytesDownloaded: 244_000_000,
          bytesTotal: 488_000_000,
        },
      },
    };

    render(<InstallingModelsPanel models={[WHISPER]} state={state} onDownload={vi.fn()} />);

    expect(screen.getByText(/Downloading 50%/)).toBeDefined();
  });

  test('shows a Retry button and error message on failed rows', () => {
    const state: ManagerState = {
      rows: { whisper: { status: 'failed', message: 'network drop' } },
    };
    const onDownload = vi.fn();

    render(<InstallingModelsPanel models={[WHISPER]} state={state} onDownload={onDownload} />);

    expect(screen.getByText('network drop')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(onDownload).toHaveBeenCalledWith('whisper');
  });

  test('shows Re-download on installed rows', () => {
    const state: ManagerState = { rows: { whisper: { status: 'installed' } } };

    render(<InstallingModelsPanel models={[WHISPER]} state={state} onDownload={vi.fn()} />);

    expect(screen.getByRole('button', { name: 'Re-download' })).toBeDefined();
  });
});
