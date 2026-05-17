import { invoke } from '@tauri-apps/api/core';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '../../shared/ui/styles.css';

const container = document.getElementById('root');
if (!container) throw new Error('Settings window: missing #root');

function App() {
  return (
    <main className="p-6 text-sm">
      <h1 className="font-semibold">Settings (placeholder)</h1>
      <div className="mt-4 grid grid-cols-2 gap-2">
        <button
          type="button"
          className="rounded border border-neutral-300 px-3 py-1"
          onClick={() => invoke('show_pill')}
        >
          Show Pill
        </button>
        <button
          type="button"
          className="rounded border border-neutral-300 px-3 py-1"
          onClick={() => invoke('hide_pill')}
        >
          Hide Pill
        </button>
        <button
          type="button"
          className="rounded border border-neutral-300 px-3 py-1"
          onClick={() => invoke('show_overlay')}
        >
          Show Overlay
        </button>
        <button
          type="button"
          className="rounded border border-neutral-300 px-3 py-1"
          onClick={() => invoke('hide_overlay')}
        >
          Hide Overlay
        </button>
      </div>
    </main>
  );
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
