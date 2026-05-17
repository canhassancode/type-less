import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '../../shared/ui/styles.css';

const container = document.getElementById('root');
if (!container) throw new Error('Pill window: missing #root');

createRoot(container).render(
  <StrictMode>
    <div className="h-full w-full" />
  </StrictMode>,
);
