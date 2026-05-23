import type { EngineState, EngineStateChanged } from '../../shared/ipc/bindings';
import type { Unbind } from '../activation/bindHotkey';

export interface SessionOrchestratorDeps {
  getEngineState: () => Promise<EngineState>;
  listen: (handler: (event: { payload: EngineStateChanged }) => void) => Promise<() => void>;
  bind: () => Promise<Unbind>;
  logger?: Pick<Console, 'error'>;
}

export async function mountSessionOrchestrator(
  deps: SessionOrchestratorDeps,
): Promise<() => Promise<void>> {
  const log = deps.logger ?? console;
  let currentUnbind: Unbind | null = null;

  async function applyState(state: EngineState): Promise<void> {
    if (state === 'Ready' && !currentUnbind) {
      try {
        currentUnbind = await deps.bind();
      } catch (error) {
        log.error('[orchestrator] bindHotkey failed', error);
      }
    } else if (state !== 'Ready' && currentUnbind) {
      const unbind = currentUnbind;
      currentUnbind = null;
      try {
        await unbind();
      } catch (error) {
        log.error('[orchestrator] unbind failed', error);
      }
    }
  }

  const unlisten = await deps.listen(({ payload }) => {
    void applyState(payload.state);
  });

  try {
    const initial = await deps.getEngineState();
    await applyState(initial);
  } catch (error) {
    log.error('[orchestrator] initial engine state query failed', error);
  }

  return async () => {
    unlisten();
    if (currentUnbind) {
      const unbind = currentUnbind;
      currentUnbind = null;
      try {
        await unbind();
      } catch (error) {
        log.error('[orchestrator] dispose unbind failed', error);
      }
    }
  };
}
