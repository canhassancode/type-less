import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '../../shared/ui/styles.css';

const container = document.getElementById('root');
if (!container) throw new Error('Pill window: missing #root');

createRoot(container).render(
  <StrictMode>
    <div className="flex h-full w-full items-center justify-center rounded-full bg-red-500/40 text-xs text-white">
      pill
    </div>
  </StrictMode>,
);
