import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '../../shared/ui/styles.css';

const container = document.getElementById('root');
if (!container) throw new Error('Settings window: missing #root');

createRoot(container).render(
  <StrictMode>
    <main className="p-6 text-sm">Settings (placeholder)</main>
  </StrictMode>,
);
