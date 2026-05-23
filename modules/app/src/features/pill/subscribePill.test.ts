import { describe, expect, test, vi } from 'vitest';
import type { DictationStateChanged } from '../../shared/ipc/bindings';
import { type PillView, subscribePill } from './subscribePill';

function flushPromises(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

interface ListenCapture {
  listen: (cb: (event: { payload: DictationStateChanged }) => void) => Promise<() => void>;
  fire: (payload: DictationStateChanged) => void;
  unlisten: ReturnType<typeof vi.fn>;
}

function makeListenCapture(): ListenCapture {
  let captured: ((event: { payload: DictationStateChanged }) => void) | undefined;
  const unlisten = vi.fn();
  const listen = async (cb: (event: { payload: DictationStateChanged }) => void) => {
    captured = cb;
    return unlisten;
  };
  function fire(payload: DictationStateChanged) {
    if (!captured) throw new Error('listen has not been called yet');
    captured({ payload });
  }
  return { listen, fire, unlisten };
}

describe('subscribePill', () => {
  test('Recording event calls show and emits a visible Recording view', async () => {
    const capture = makeListenCapture();
    const show = vi.fn(async () => {});
    const hide = vi.fn(async () => {});
    const views: PillView[] = [];

    await subscribePill({
      listen: capture.listen,
      show,
      hide,
      onView: (view) => views.push(view),
    });

    capture.fire({ stage: 'Recording' });
    await Promise.resolve();

    expect(show).toHaveBeenCalledOnce();
    expect(hide).not.toHaveBeenCalled();
    expect(views).toEqual([{ visible: true, stage: 'Recording' }]);
  });

  test('Loading event keeps Pill visible, no show/hide IPC', async () => {
    const capture = makeListenCapture();
    const show = vi.fn(async () => {});
    const hide = vi.fn(async () => {});
    const views: PillView[] = [];

    await subscribePill({
      listen: capture.listen,
      show,
      hide,
      onView: (view) => views.push(view),
    });

    capture.fire({ stage: 'Loading' });
    await Promise.resolve();

    expect(show).not.toHaveBeenCalled();
    expect(hide).not.toHaveBeenCalled();
    expect(views).toEqual([{ visible: true, stage: 'Loading' }]);
  });

  test('Idle event calls hide and emits a hidden view', async () => {
    const capture = makeListenCapture();
    const show = vi.fn(async () => {});
    const hide = vi.fn(async () => {});
    const views: PillView[] = [];

    await subscribePill({
      listen: capture.listen,
      show,
      hide,
      onView: (view) => views.push(view),
    });

    capture.fire({ stage: 'Idle' });
    await Promise.resolve();

    expect(hide).toHaveBeenCalledOnce();
    expect(show).not.toHaveBeenCalled();
    expect(views).toEqual([{ visible: false, stage: null }]);
  });

  test('full Recording → Loading → Idle cycle drives ports + views in order', async () => {
    const capture = makeListenCapture();
    const show = vi.fn(async () => {});
    const hide = vi.fn(async () => {});
    const views: PillView[] = [];

    await subscribePill({
      listen: capture.listen,
      show,
      hide,
      onView: (view) => views.push(view),
    });

    capture.fire({ stage: 'Recording' });
    await flushPromises();
    capture.fire({ stage: 'Loading' });
    await flushPromises();
    capture.fire({ stage: 'Idle' });
    await flushPromises();

    expect(show).toHaveBeenCalledOnce();
    expect(hide).toHaveBeenCalledOnce();
    expect(views).toEqual([
      { visible: true, stage: 'Recording' },
      { visible: true, stage: 'Loading' },
      { visible: false, stage: null },
    ]);
  });

  test('returned unbind invokes the listener unlisten fn', async () => {
    const capture = makeListenCapture();
    const unbind = await subscribePill({
      listen: capture.listen,
      show: async () => {},
      hide: async () => {},
      onView: () => {},
    });

    await unbind();

    expect(capture.unlisten).toHaveBeenCalledOnce();
  });
});
