import { register, unregister } from '@tauri-apps/plugin-global-shortcut';
import { commands, type Result } from '../../shared/ipc/bindings';
import { bindHotkey, type Unbind } from './bindHotkey';

export const TRACER_HOTKEY = 'CommandOrControl+Shift+.';

function unwrap(result: Result<null, string>): void {
  if (result.status === 'error') {
    throw new Error(result.error);
  }
}

export async function mountActivation(): Promise<Unbind> {
  return bindHotkey({
    register,
    unregister,
    hotkey: TRACER_HOTKEY,
    onPress: async () => unwrap(await commands.startDictationSession()),
    onRelease: async () => unwrap(await commands.endDictationSession()),
  });
}
