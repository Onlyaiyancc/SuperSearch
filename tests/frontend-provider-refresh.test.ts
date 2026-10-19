import { afterEach, describe, expect, it, vi } from 'vitest';
import { watchLoadingProvider } from '../src/provider-refresh';
import type { Provider } from '../src/types';

const status = (value: Provider['status']): Provider[] => [{ id: 'everything', status: value, message: '' }];
afterEach(() => vi.useRealTimers());

describe('visible loading provider refresh', () => {
  it('refreshes once per second while loading and stops when ready', async () => {
    vi.useFakeTimers();
    const refresh = vi.fn().mockResolvedValueOnce(status('indexLoading')).mockResolvedValueOnce(status('loading')).mockResolvedValue(status('ready'));
    const stop = watchLoadingProvider(refresh);
    expect(refresh).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(999);
    expect(refresh).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(2001);
    expect(refresh).toHaveBeenCalledTimes(3);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(refresh).toHaveBeenCalledTimes(3);
    stop();
  });

  it('cancels a scheduled refresh on leaving the visible settings page', async () => {
    vi.useFakeTimers();
    const refresh = vi.fn().mockResolvedValue(status('indexLoading'));
    const stop = watchLoadingProvider(refresh);
    stop();
    stop();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(refresh).not.toHaveBeenCalled();
  });

  it('serializes slow probes and invalidates a reply after disposal', async () => {
    vi.useFakeTimers();
    let resolve!: (value: Provider[]) => void;
    let current!: () => boolean;
    const refresh = vi.fn((isCurrent: () => boolean) => {
      current = isCurrent;
      return new Promise<Provider[]>(done => { resolve = done; });
    });
    const stop = watchLoadingProvider(refresh);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(current()).toBe(true);
    stop();
    expect(current()).toBe(false);
    resolve(status('indexLoading'));
    await vi.advanceTimersByTimeAsync(60_000);
    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it.each(['unavailable', 'notRunning', 'notConfigured'] as const)('stops on terminal status %s', async value => {
    vi.useFakeTimers();
    const refresh = vi.fn().mockResolvedValue(status(value));
    const stop = watchLoadingProvider(refresh);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(refresh).toHaveBeenCalledTimes(1);
    stop();
  });

  it('stops when the request fails', async () => {
    vi.useFakeTimers();
    for (const refresh of [vi.fn().mockRejectedValue(new Error('Disconnected')), vi.fn().mockResolvedValue([])]) {
      const stop = watchLoadingProvider(refresh);
      await vi.advanceTimersByTimeAsync(60_000);
      expect(refresh).toHaveBeenCalledTimes(1);
      stop();
    }
  });

  it('keeps checking when another status request supersedes a probe', async () => {
    vi.useFakeTimers();
    const refresh = vi.fn().mockResolvedValueOnce(undefined).mockResolvedValue(status('ready'));
    const stop = watchLoadingProvider(refresh);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(refresh).toHaveBeenCalledTimes(2);
    stop();
  });

  it('does not poll the file provider just because the application catalog is loading', async () => {
    vi.useFakeTimers();
    const refresh = vi.fn().mockResolvedValue([{ id: 'apps', status: 'loading', message: '' }, ...status('ready')]);
    const stop = watchLoadingProvider(refresh);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(refresh).toHaveBeenCalledTimes(1);
    stop();
  });
});
