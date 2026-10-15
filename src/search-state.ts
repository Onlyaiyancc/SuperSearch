import type { Provider } from './types';

export function mergeProviders(current: Provider[], incoming: Provider[]) {
  // 后到状态覆盖同一来源，局部快照省略的依赖错误继续保留。
  return [...new Map([...current, ...incoming].map(provider => [provider.id, provider])).values()];
}

export const scopes = ['all', 'apps', 'files', 'folders'] as const;
export function nextRequestId(session: string, epoch: number) { return `${session}-${epoch}`; }
type SnapshotIdentity = { requestId: string; snapshotRevision: number; scope: string };

export function acceptSnapshot(current: SnapshotIdentity, incoming: SnapshotIdentity) {
  // 请求身份已经绑定范围；后端可将 app:/file:/folder: 前缀解析为更窄的有效范围。
  return incoming.requestId === current.requestId && incoming.snapshotRevision > current.snapshotRevision;
}

export function selectedEntity(items: { entityId: string }[], previous: string | null, hadSelection: boolean): string | null {
  // 自动默认项跟随最新排序；只有用户明确选择的实体才保持不变。
  if (!hadSelection) return items[0]?.entityId ?? null;
  if (previous && items.some(item => item.entityId === previous)) return previous;
  // 已选实体消失时清空选择，避免相同行号指向另一个目标。
  return null;
}

export function nextScope(scope: string, backwards = false) {
  return scopes[(scopes.indexOf(scope as typeof scopes[number]) + (backwards ? 3 : 1)) % scopes.length];
}

export function blocksIme(event: { isComposing?: boolean; keyCode?: number }, composing: boolean, endedAt: number, now: number) {
  return composing || !!event.isComposing || event.keyCode === 229 || (endedAt > 0 && now - endedAt < 100);
}

export function hasDirectoryQuery(query: string) {
  return /[\\/]/.test(query) || /(?:^|\s)in:/i.test(query);
}

export function enterAction(event: { repeat?: boolean; ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean; metaKey?: boolean }, state: { query: string; busy: boolean; imeBlocked: boolean; pending: boolean; complete: boolean; hasMatches: boolean; available: boolean }): 'web' | 'open' | 'reveal' | null {
  if (!state.query.trim() || state.busy || state.imeBlocked || event.repeat || event.altKey || event.metaKey || (event.ctrlKey && event.shiftKey)) return null;
  // Shift+Enter 明确要求网页搜索，不等待本地结果；Ctrl+Enter 只定位本地项目。
  if (event.shiftKey) return 'web';
  if (state.pending) return null;
  if (state.hasMatches) return state.available ? event.ctrlKey ? 'reveal' : 'open' : null;
  return !event.ctrlKey && state.complete && !hasDirectoryQuery(state.query) ? 'web' : null;
}
