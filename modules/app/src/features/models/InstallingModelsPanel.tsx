import type { Model } from '../../shared/ipc/bindings';
import type { ManagerState, ModelRowState } from './downloadManager';
import { formatBytes } from './formatBytes';

interface InstallingModelsPanelProps {
  models: Model[];
  state: ManagerState;
  onDownload: (modelId: string) => void;
}

export function InstallingModelsPanel({ models, state, onDownload }: InstallingModelsPanelProps) {
  return (
    <section className="space-y-3">
      <header>
        <h2 className="text-base font-semibold">Models</h2>
        <p className="text-xs text-neutral-500">
          Local language models for transcription and cleanup. One-time download.
        </p>
      </header>
      <ul className="divide-y divide-neutral-200 rounded border border-neutral-200">
        {models.map((model) => {
          const row = state.rows[model.id] ?? { status: 'not-installed' };
          return <ModelRow key={model.id} model={model} row={row} onDownload={onDownload} />;
        })}
      </ul>
    </section>
  );
}

interface ModelRowProps {
  model: Model;
  row: ModelRowState;
  onDownload: (modelId: string) => void;
}

function ModelRow({ model, row, onDownload }: ModelRowProps) {
  return (
    <li className="flex items-center justify-between gap-3 px-3 py-2">
      <div className="min-w-0">
        <div className="truncate font-mono text-xs">{model.filename}</div>
        <div className="text-[11px] text-neutral-500">
          {model.purpose} · {formatBytes(model.size_bytes)}
        </div>
        {row.status === 'downloading' && (
          <DownloadingDetail row={row} fallbackTotal={model.size_bytes} />
        )}
      </div>
      <RowAction row={row} onClick={() => onDownload(model.id)} />
    </li>
  );
}

function DownloadingDetail({
  row,
  fallbackTotal,
}: {
  row: Extract<ModelRowState, { status: 'downloading' }>;
  fallbackTotal: number;
}) {
  const total = row.bytesTotal || fallbackTotal;
  const percent = total > 0 ? Math.min(100, Math.round((row.bytesDownloaded / total) * 100)) : 0;
  return (
    <div className="mt-1">
      <div className="text-[11px] text-neutral-500">
        Downloading {percent}% · {formatBytes(row.bytesDownloaded)} / {formatBytes(total)}
      </div>
      <div className="mt-1 h-1 w-48 overflow-hidden rounded bg-neutral-200">
        <div className="h-full bg-neutral-700" style={{ width: `${percent}%` }} />
      </div>
    </div>
  );
}

function RowAction({ row, onClick }: { row: ModelRowState; onClick: () => void }) {
  switch (row.status) {
    case 'not-installed':
      return <ActionButton label="Download" onClick={onClick} />;
    case 'downloading':
      return <span className="text-[11px] text-neutral-500">…</span>;
    case 'installed':
      return <ActionButton label="Re-download" onClick={onClick} variant="ghost" />;
    case 'failed':
      return (
        <div className="flex flex-col items-end gap-1">
          <span className="text-[11px] text-rose-600">{row.message}</span>
          <ActionButton label="Retry" onClick={onClick} />
        </div>
      );
  }
}

function ActionButton({
  label,
  onClick,
  variant = 'primary',
}: {
  label: string;
  onClick: () => void;
  variant?: 'primary' | 'ghost';
}) {
  const styles =
    variant === 'primary'
      ? 'border-neutral-700 bg-neutral-900 text-white hover:bg-neutral-700'
      : 'border-neutral-300 text-neutral-700 hover:bg-neutral-100';
  return (
    <button
      type="button"
      className={`rounded border px-3 py-1 text-xs transition-colors ${styles}`}
      onClick={onClick}
    >
      {label}
    </button>
  );
}
