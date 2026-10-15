import type { Scope, SearchSnapshot } from './types';

export type WindowLayout = { layout: 'compact' | 'results' | 'settings'; rows: number; requestId?: string };

export function emptySearchSession(requestId: string) {
  return {
    query: '', scope: 'all' as Scope, composing: false, compositionEndedAt: 0,
    selected: null, menu: null, notice: '', error: '', pending: false,
    snapshot: { requestId, snapshotRevision: -1, scope: 'all', complete: true, items: [], providers: [], truncated: false, hasMore: false } as SearchSnapshot,
  };
}

export function searchLayout(query: string, requestId: string, snapshot: { requestId: string; items: unknown[]; folderCompletions?: unknown[] }, pending: boolean, settings: boolean): WindowLayout {
  if (settings) return { layout: 'settings', rows: 0 };
  const count = snapshot.items.length + Math.min(snapshot.folderCompletions?.length ?? 0, 5);
  const matches = !!query.trim() && snapshot.requestId === requestId && !pending && count > 0;
  return matches ? { layout: 'results', rows: Math.min(count, 6), requestId } : { layout: 'compact', rows: 0 };
}

export function menuPosition(anchor: { right: number; top: number; bottom: number }, menu: { width: number; height: number }, viewport: { width: number; height: number }) {
  const below = anchor.bottom + 5;
  const top = below + menu.height <= viewport.height - 8 ? below : anchor.top - menu.height - 5;
  return {
    left: Math.max(8, Math.min(anchor.right - menu.width, viewport.width - menu.width - 8)),
    top: Math.max(8, Math.min(top, viewport.height - menu.height - 8)),
  };
}

export function createLayoutSync(send: (value: WindowLayout) => Promise<unknown>) {
  type Waiter = { resolve: () => void; reject: (value: unknown) => void };
  let next: { value: WindowLayout; waiters: Waiter[] } | undefined;
  let running = false;
  let applied = '';
  return (value: WindowLayout): Promise<void> => {
    return new Promise<void>((resolve, reject) => {
      if (next) {
        next.value = value;
        next.waiters.push({ resolve, reject });
      } else next = { value, waiters: [{ resolve, reject }] };
      if (running) return;
      running = true;
      // 串行发送尺寸，只保留等待中的最新布局，避免后到的旧请求覆盖新尺寸。
      void (async () => {
        while (next) {
          const batch = next;
          next = undefined;
          const target = batch.value;
          const key = `${target.layout}:${target.rows}:${target.requestId ?? ''}`;
          try {
            if (key !== applied) {
              await send(target);
              applied = key;
            }
            batch.waiters.forEach(waiter => waiter.resolve());
          } catch (value) {
            // 只拒绝本次发送对应的等待者；旧尺寸失败后继续发送已排队的新尺寸。
            batch.waiters.forEach(waiter => waiter.reject(value));
          }
        }
        // 结束和清除运行标记同步完成，重复尺寸不会困住随后到达的变更。
        running = false;
      })();
    });
  };
}
