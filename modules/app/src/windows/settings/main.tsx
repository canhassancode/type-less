import { invoke } from '@tauri-apps/api/core';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { mountActivation } from '../../features/activation';
import { InstallingModelsPanel, useInstallingModels } from '../../features/models';
import '../../shared/ui/styles.css';

const container = document.getElementById('root');
if (!container) throw new Error('Settings window: missing #root');

const activationPromise = mountActivation().catch((error) => {
  console.error('[activation] failed to register hotkey', error);
  return undefined;
});

if (import.meta.hot) {
  import.meta.hot.dispose(async () => {
    const unbind = await activationPromise;
    if (unbind) await unbind();
  });
}

function SettingsApp() {
  const { models, state, downloadOne } = useInstallingModels();

  return (
    <main className="space-y-6 p-6 text-sm">
      <header>
        <h1 className="text-lg font-semibold">Settings</h1>
      </header>

      <InstallingModelsPanel
        models={models}
        state={state}
        onDownload={(id) => {
          void downloadOne(id);
        }}
      />

      <details className="rounded border border-neutral-200 p-3 text-xs">
        <summary className="cursor-pointer font-medium text-neutral-600">Developer tools</summary>
        <div className="mt-3 grid grid-cols-2 gap-2">
          <DevButton label="Show Pill" onClick={() => invoke('show_pill')} />
          <DevButton label="Hide Pill" onClick={() => invoke('hide_pill')} />
          <DevButton label="Show Overlay" onClick={() => invoke('show_overlay')} />
          <DevButton label="Hide Overlay" onClick={() => invoke('hide_overlay')} />
        </div>
      </details>
    </main>
  );
}

function DevButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      className="rounded border border-neutral-300 px-3 py-1 hover:bg-neutral-100"
      onClick={onClick}
    >
      {label}
    </button>
  );
}

createRoot(container).render(
  <StrictMode>
    <SettingsApp />
  </StrictMode>,
);
