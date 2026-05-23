import { commands, events } from '../../shared/ipc/bindings';
import { mountActivation } from '../activation';
import { mountSessionOrchestrator } from './sessionOrchestrator';

export async function mountSessionOrchestratorWithDefaults(): Promise<() => Promise<void>> {
  return mountSessionOrchestrator({
    getEngineState: async () => {
      const result = await commands.engineState();
      if (result.status === 'error') {
        throw new Error(result.error);
      }
      return result.data;
    },
    listen: events.engineStateChanged.listen,
    bind: mountActivation,
  });
}
