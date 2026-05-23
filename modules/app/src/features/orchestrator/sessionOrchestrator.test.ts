import { describe, expect, test, vi } from 'vitest';
import type { EngineState, EngineStateChanged } from '../../shared/ipc/bindings';
import { mountSessionOrchestrator } from './sessionOrchestrator';

function flushPromises(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

interface Harness {
  getEngineState: ReturnType<typeof vi.fn>;
  listen: ReturnType<typeof vi.fn>;
  fire: (payload: EngineStateChanged) => void;
  unlisten: ReturnType<typeof vi.fn>;
  bind: ReturnType<typeof vi.fn>;
  unbind: ReturnType<typeof vi.fn>;
}

function makeHarness(initialState: EngineState): Harness {
  let captured: ((event: { payload: EngineStateChanged }) => void) | undefined;
  const unlisten = vi.fn();
  const unbind = vi.fn(async () => {});
  const getEngineState = vi.fn(async () => initialState);
  const listen = vi.fn(async (cb: (event: { payload: EngineStateChanged }) => void) => {
    captured = cb;
    return unlisten;
  });
  const bind = vi.fn(async () => unbind);
  function fire(payload: EngineStateChanged) {
    if (!captured) throw new Error('listen not called yet');
    captured({ payload });
  }
  return { getEngineState, listen, fire, unlisten, bind, unbind };
}

describe('mountSessionOrchestrator', () => {
  test('binds hotkey immediately when initial state is Ready', async () => {
    const h = makeHarness('Ready');

    await mountSessionOrchestrator({
      getEngineState: h.getEngineState,
      listen: h.listen,
      bind: h.bind,
    });

    expect(h.bind).toHaveBeenCalledOnce();
  });

  test('does not bind when initial state is Loading', async () => {
    const h = makeHarness('Loading');

    await mountSessionOrchestrator({
      getEngineState: h.getEngineState,
      listen: h.listen,
      bind: h.bind,
    });

    expect(h.bind).not.toHaveBeenCalled();
  });

  test('does not bind when initial state is Degraded', async () => {
    const h = makeHarness('Degraded');

    await mountSessionOrchestrator({
      getEngineState: h.getEngineState,
      listen: h.listen,
      bind: h.bind,
    });

    expect(h.bind).not.toHaveBeenCalled();
  });

  test('Ready event after Loading triggers bind exactly once', async () => {
    const h = makeHarness('Loading');

    await mountSessionOrchestrator({
      getEngineState: h.getEngineState,
      listen: h.listen,
      bind: h.bind,
    });

    h.fire({ state: 'Ready' });
    await flushPromises();

    expect(h.bind).toHaveBeenCalledOnce();
  });

  test('Loading event while bound triggers unbind', async () => {
    const h = makeHarness('Ready');

    await mountSessionOrchestrator({
      getEngineState: h.getEngineState,
      listen: h.listen,
      bind: h.bind,
    });
    await flushPromises();

    h.fire({ state: 'Loading' });
    await flushPromises();

    expect(h.unbind).toHaveBeenCalledOnce();
  });

  test('Degraded event while bound triggers unbind', async () => {
    const h = makeHarness('Ready');

    await mountSessionOrchestrator({
      getEngineState: h.getEngineState,
      listen: h.listen,
      bind: h.bind,
    });
    await flushPromises();

    h.fire({ state: 'Degraded' });
    await flushPromises();

    expect(h.unbind).toHaveBeenCalledOnce();
  });

  test('redundant Ready events while already bound do not double-bind', async () => {
    const h = makeHarness('Ready');

    await mountSessionOrchestrator({
      getEngineState: h.getEngineState,
      listen: h.listen,
      bind: h.bind,
    });
    await flushPromises();

    h.fire({ state: 'Ready' });
    h.fire({ state: 'Ready' });
    await flushPromises();

    expect(h.bind).toHaveBeenCalledOnce();
  });

  test('Loading after Loading does not call unbind twice', async () => {
    const h = makeHarness('Loading');

    await mountSessionOrchestrator({
      getEngineState: h.getEngineState,
      listen: h.listen,
      bind: h.bind,
    });

    h.fire({ state: 'Loading' });
    h.fire({ state: 'Loading' });
    await flushPromises();

    expect(h.unbind).not.toHaveBeenCalled();
  });

  test('dispose unlistens and unbinds if currently bound', async () => {
    const h = makeHarness('Ready');

    const dispose = await mountSessionOrchestrator({
      getEngineState: h.getEngineState,
      listen: h.listen,
      bind: h.bind,
    });
    await flushPromises();

    await dispose();

    expect(h.unlisten).toHaveBeenCalledOnce();
    expect(h.unbind).toHaveBeenCalledOnce();
  });

  test('dispose only unlistens when not currently bound', async () => {
    const h = makeHarness('Loading');

    const dispose = await mountSessionOrchestrator({
      getEngineState: h.getEngineState,
      listen: h.listen,
      bind: h.bind,
    });

    await dispose();

    expect(h.unlisten).toHaveBeenCalledOnce();
    expect(h.unbind).not.toHaveBeenCalled();
  });
});
