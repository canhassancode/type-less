import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { type PillView, subscribePill } from '../../features/pill/subscribePill';
import { commands, events } from '../../shared/ipc/bindings';
import '../../shared/ui/styles.css';

const HIDDEN_VIEW: PillView = { visible: false, stage: null };

function Pill() {
  const [view, setView] = useState<PillView>(HIDDEN_VIEW);

  useEffect(() => {
    let unbind: (() => Promise<void>) | undefined;
    let cancelled = false;

    subscribePill({
      listen: events.dictationStateChanged.listen,
      show: async () => {
        await commands.showPill();
      },
      hide: async () => {
        await commands.hidePill();
      },
      onView: setView,
    }).then((u) => {
      if (cancelled) {
        void u();
      } else {
        unbind = u;
      }
    });

    return () => {
      cancelled = true;
      if (unbind) void unbind();
    };
  }, []);

  if (!view.visible) return null;

  const stageStyles =
    view.stage === 'Recording'
      ? 'bg-red-500/50 text-white'
      : 'bg-zinc-500/60 text-white animate-pulse';
  const label = view.stage === 'Recording' ? 'REC' : '...';

  return (
    <div
      className={`flex h-full w-full items-center justify-center rounded-full text-xs ${stageStyles}`}
    >
      {label}
    </div>
  );
}

const container = document.getElementById('root');
if (!container) throw new Error('Pill window: missing #root');

createRoot(container).render(
  <StrictMode>
    <Pill />
  </StrictMode>,
);
