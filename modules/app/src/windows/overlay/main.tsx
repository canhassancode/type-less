import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '../../shared/ui/styles.css';

const container = document.getElementById('root');
if (!container) throw new Error('Overlay window: missing #root');

createRoot(container).render(
  <StrictMode>
    <div className="pointer-events-none flex h-full w-full items-center justify-center" />
  </StrictMode>,
);
