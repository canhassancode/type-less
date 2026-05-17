export type ShortcutState = 'Pressed' | 'Released';

export interface ShortcutEvent {
  shortcut: string;
  state: ShortcutState;
}

export type ShortcutHandler = (event: ShortcutEvent) => void | Promise<void>;

export type RegisterFn = (shortcut: string, handler: ShortcutHandler) => Promise<void>;

export type UnregisterFn = (shortcut: string) => Promise<void>;

export type Unbind = () => Promise<void>;

export interface BindHotkeyDeps {
  register: RegisterFn;
  unregister: UnregisterFn;
  hotkey: string;
  onPress: () => Promise<void>;
  onRelease: () => Promise<void>;
  logger?: Pick<Console, 'error'>;
}

export async function bindHotkey(deps: BindHotkeyDeps): Promise<Unbind> {
  const log = deps.logger ?? console;
  await deps.register(deps.hotkey, async (event) => {
    try {
      if (event.state === 'Pressed') {
        await deps.onPress();
      } else {
        await deps.onRelease();
      }
    } catch (error) {
      log.error('[activation] hotkey handler failed', error);
    }
  });
  return () => deps.unregister(deps.hotkey);
}
