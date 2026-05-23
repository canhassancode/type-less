import type { DictationStateChanged } from '../../shared/ipc/bindings';

export interface PillView {
  visible: boolean;
  stage: 'Recording' | 'Loading' | null;
}

export interface SubscribePillDeps {
  listen: (handler: (event: { payload: DictationStateChanged }) => void) => Promise<() => void>;
  show: () => Promise<void>;
  hide: () => Promise<void>;
  onView: (view: PillView) => void;
  logger?: Pick<Console, 'error'>;
}

export async function subscribePill(deps: SubscribePillDeps): Promise<() => Promise<void>> {
  const log = deps.logger ?? console;
  const unlisten = await deps.listen(({ payload }) => {
    void handleStage(payload.stage, deps, log);
  });
  return async () => {
    unlisten();
  };
}

async function handleStage(
  stage: DictationStateChanged['stage'],
  deps: SubscribePillDeps,
  log: Pick<Console, 'error'>,
): Promise<void> {
  try {
    switch (stage) {
      case 'Recording':
        await deps.show();
        deps.onView({ visible: true, stage: 'Recording' });
        return;
      case 'Loading':
        deps.onView({ visible: true, stage: 'Loading' });
        return;
      case 'Idle':
        await deps.hide();
        deps.onView({ visible: false, stage: null });
        return;
    }
  } catch (error) {
    log.error('[pill] failed to handle stage', stage, error);
  }
}
