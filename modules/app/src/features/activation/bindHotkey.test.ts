import { describe, expect, test, vi } from 'vitest';
import { bindHotkey, type ShortcutHandler } from './bindHotkey';

function makeRegisterCapture() {
  let captured: ShortcutHandler | undefined;
  const register = vi.fn(async (_shortcut: string, handler: ShortcutHandler) => {
    captured = handler;
  });
  function handler(): ShortcutHandler {
    if (!captured) {
      throw new Error('register has not been called yet');
    }
    return captured;
  }
  return { register, handler };
}

describe('bindHotkey', () => {
  test('routes Pressed events to onPress and Released to onRelease', async () => {
    const { register, handler } = makeRegisterCapture();
    const unregister = vi.fn(async () => {});
    const onPress = vi.fn(async () => {});
    const onRelease = vi.fn(async () => {});

    await bindHotkey({ register, unregister, hotkey: 'Alt', onPress, onRelease });

    expect(register).toHaveBeenCalledWith('Alt', expect.any(Function));

    await handler()({ shortcut: 'Alt', state: 'Pressed' });
    await handler()({ shortcut: 'Alt', state: 'Released' });

    expect(onPress).toHaveBeenCalledOnce();
    expect(onRelease).toHaveBeenCalledOnce();
  });

  test('swallows handler errors and logs them so the listener survives', async () => {
    const { register, handler } = makeRegisterCapture();
    const unregister = vi.fn(async () => {});
    const failure = new Error('start_dictation_session rejected');
    const onPress = vi.fn(async () => {
      throw failure;
    });
    const onRelease = vi.fn(async () => {});
    const logger = { error: vi.fn() };

    await bindHotkey({ register, unregister, hotkey: 'Alt', onPress, onRelease, logger });

    await expect(handler()({ shortcut: 'Alt', state: 'Pressed' })).resolves.toBeUndefined();

    expect(logger.error).toHaveBeenCalledWith(expect.any(String), failure);

    await handler()({ shortcut: 'Alt', state: 'Released' });
    expect(onRelease).toHaveBeenCalledOnce();
  });

  test('returned unbind calls unregister with the bound hotkey', async () => {
    const { register } = makeRegisterCapture();
    const unregister = vi.fn(async () => {});

    const unbind = await bindHotkey({
      register,
      unregister,
      hotkey: 'Alt',
      onPress: async () => {},
      onRelease: async () => {},
    });

    await unbind();

    expect(unregister).toHaveBeenCalledWith('Alt');
  });
});
