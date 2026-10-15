import { describe, expect, it } from 'vitest';
import { acceptSnapshot, selectedEntity, nextScope, blocksIme, nextRequestId, mergeProviders } from '../src/search-state';
import type { Provider } from '../src/types';

describe('provider status retention', () => {
  const apps: Provider = { id: 'apps', status: 'ready', message: '4 applications' };
  const dependency: Provider = { id: 'everything', status: 'notConfigured', message: 'Choose Everything.exe' };
  const shortcut: Provider = { id: 'shortcut', status: 'unavailable', message: 'Shortcut is unavailable' };

  it('retains dependency and shortcut failures omitted from an apps snapshot', () => {
    expect(mergeProviders([dependency, shortcut], [apps])).toEqual([dependency, shortcut, apps]);
  });

  it('replaces the status of the same provider when it recovers', () => {
    const ready: Provider = { ...dependency, status: 'ready', message: 'Everything is ready' };
    expect(mergeProviders([dependency, shortcut], [ready])).toEqual([ready, shortcut]);
    expect(dependency.status).toBe('notConfigured');
  });

  it('uses the final update per provider without duplicates', () => {
    const failed: Provider = { ...apps, status: 'unavailable', message: 'Unable to discover applications' };
    expect(mergeProviders([apps], [failed, apps])).toEqual([apps]);
    expect(mergeProviders([dependency, shortcut], [])).toEqual([dependency, shortcut]);
  });
});

describe('search snapshot safety', () => {
  it('generates session-qualified string request IDs for native IPC', () => {
    expect(nextRequestId('session-a', 2)).toBe('session-a-2');
    expect(nextRequestId('session-b', 2)).not.toBe(nextRequestId('session-a', 2));
  });
  const current = { requestId: 'q-4', snapshotRevision: 1, scope: 'all' };
  it('accepts only a newer revision of the active request', () => {
    expect(acceptSnapshot(current, { ...current, snapshotRevision: 2 })).toBe(true);
    expect(acceptSnapshot(current, { ...current, requestId: 'q-3', snapshotRevision: 8 })).toBe(false);
    expect(acceptSnapshot(current, { ...current, requestId: 'other', scope: 'files', snapshotRevision: 2 })).toBe(false);
    expect(acceptSnapshot(current, current)).toBe(false);
  });
  it('accepts a prefix-filtered effective scope for the same request', () => {
    expect(acceptSnapshot(current, { ...current, scope: 'apps', snapshotRevision: 2 })).toBe(true);
    expect(acceptSnapshot(current, { ...current, scope: 'files', snapshotRevision: 2 })).toBe(true);
    expect(acceptSnapshot(current, { ...current, scope: 'folders', snapshotRevision: 2 })).toBe(true);
  });
  it('keeps the selected entity when late results shift its row', () => {
    expect(selectedEntity([{ entityId: 'file' }, { entityId: 'app' }], 'app', true)).toBe('app');
  });
  it('updates the automatic default to the first final result until the user selects explicitly', () => {
    expect(selectedEntity([{ entityId: 'exact' }, { entityId: 'prefix' }], 'prefix', false)).toBe('exact');
    expect(selectedEntity([{ entityId: 'exact' }, { entityId: 'prefix' }], 'prefix', true)).toBe('prefix');
  });
  it('clears a disappeared selection instead of launching the row that replaces it', () => {
    expect(selectedEntity([{ entityId: 'other' }], 'app', true)).toBe(null);
    expect(selectedEntity([{ entityId: 'app' }], null, false)).toBe('app');
    expect(selectedEntity([], null, false)).toBe(null);
  });
  it('cycles scopes in both directions', () => {
    expect(nextScope('folders')).toBe('all');
    expect(nextScope('all', true)).toBe('folders');
    expect(nextScope('apps')).toBe('files');
  });
  it('blocks composing Enter, legacy IME key events, and the post-composition confirmation', () => {
    expect(blocksIme({ isComposing: true }, false, 0, 1000)).toBe(true);
    expect(blocksIme({ keyCode: 229 }, false, 0, 1000)).toBe(true);
    expect(blocksIme({}, true, 0, 1000)).toBe(true);
    expect(blocksIme({}, false, 1000, 1050)).toBe(true);
    expect(blocksIme({}, false, 1000, 1200)).toBe(false);
  });
});
