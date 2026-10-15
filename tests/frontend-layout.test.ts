import { describe, expect, it } from 'vitest';
import { emptySearchSession, searchLayout, createLayoutSync, menuPosition } from '../src/search-layout';
import { localizeMessage, providerMessage } from '../src/localization';

describe('compact search lifecycle', () => {
  it('resets every transient state and binds an empty snapshot to a new request', () => {
    const state = emptySearchSession('session-2');
    expect(state).toMatchObject({ query: '', scope: 'all', composing: false, compositionEndedAt: 0, selected: null, menu: null, notice: '', error: '', pending: false });
    expect(state.snapshot).toMatchObject({ requestId: 'session-2', scope: 'all', items: [], providers: [], snapshotRevision: -1 });
  });
  it('expands only a nonempty query with current settled matches', () => {
    const snapshot = { ...emptySearchSession('new').snapshot, items: [{ entityId: 'app' }] };
    expect(searchLayout('', 'new', snapshot, false, false)).toEqual({ layout: 'compact', rows: 0 });
    expect(searchLayout('  ', 'new', snapshot, false, false).layout).toBe('compact');
    expect(searchLayout('app', 'old', snapshot, false, false).layout).toBe('compact');
    expect(searchLayout('app', 'new', snapshot, true, false).layout).toBe('compact');
    expect(searchLayout('app', 'new', snapshot, false, false)).toEqual({ layout: 'results', rows: 1, requestId: 'new' });
    expect(searchLayout('app', 'new', { ...snapshot, items: [] }, false, false).layout).toBe('compact');
    expect(searchLayout('', 'new', snapshot, true, true)).toEqual({ layout: 'settings', rows: 0 });
  });
  it('caps the result viewport at six rows', () => {
    const snapshot = { ...emptySearchSession('new').snapshot, items: Array.from({ length: 10 }, () => ({})) };
    expect(searchLayout('app', 'new', snapshot, false, false)).toEqual({ layout: 'results', rows: 6, requestId: 'new' });
  });
  it('counts folder candidates inside the same six-row budget', () => {
    const snapshot = { ...emptySearchSession('new').snapshot, folderCompletions: [{ title: '项目', path: 'D:\\项目', query: 'D:\\项目\\' }] };
    expect(searchLayout('D:/项', 'new', snapshot, false, false)).toEqual({ layout: 'results', rows: 1, requestId: 'new' });
    expect(searchLayout('D:/项', 'old', snapshot, false, false).layout).toBe('compact');
    expect(searchLayout('D:/项', 'new', snapshot, true, false).layout).toBe('compact');
    expect(searchLayout('D:/项', 'new', { ...snapshot, items: Array(8).fill({}) }, false, false).rows).toBe(6);
    expect(searchLayout('D:/项', 'new', { ...snapshot, folderCompletions: [] }, false, false).layout).toBe('compact');
  });
  it('serializes native resize calls and coalesces intermediate sizes', async () => {
    const calls: string[] = [];
    let release!: () => void;
    const sync = createLayoutSync(async value => {
      calls.push(value.layout);
      if (calls.length === 1) await new Promise<void>(resolve => { release = resolve; });
    });
    const first = sync({ layout: 'results', rows: 4 });
    const second = sync({ layout: 'settings', rows: 0 });
    const third = sync({ layout: 'compact', rows: 0 });
    expect(calls).toEqual(['results']);
    release();
    await Promise.all([first, second, third]);
    expect(calls).toEqual(['results', 'compact']);
    await sync({ layout: 'compact', rows: 0 });
    expect(calls).toEqual(['results', 'compact']);
  });
  it('allows retry after a failed native resize', async () => {
    let attempts = 0;
    const sync = createLayoutSync(async () => { if (++attempts === 1) throw new Error('native failure'); });
    await expect(sync({ layout: 'settings', rows: 0 })).rejects.toThrow('native failure');
    await sync({ layout: 'settings', rows: 0 });
    expect(attempts).toBe(2);
  });
  it('continues the latest queued resize when the in-flight resize fails', async () => {
    const calls: string[] = [];
    let rejectFirst!: (value: unknown) => void;
    const sync = createLayoutSync(async value => {
      calls.push(value.layout);
      if (calls.length === 1) await new Promise<void>((_, reject) => { rejectFirst = reject; });
    });
    const first = sync({ layout: 'results', rows: 4, requestId: 'old' });
    const failed = expect(first).rejects.toThrow('native failure');
    const latest = sync({ layout: 'compact', rows: 0 });
    rejectFirst(new Error('native failure'));
    await failed;
    await expect(latest).resolves.toBeUndefined();
    expect(calls).toEqual(['results', 'compact']);
  });
  it('does not strand a changed layout immediately after a duplicate', async () => {
    const calls: string[] = [];
    const sync = createLayoutSync(async value => { calls.push(value.layout); });
    await sync({ layout: 'compact', rows: 0 });
    const duplicate = sync({ layout: 'compact', rows: 0 });
    const changed = sync({ layout: 'settings', rows: 0 });
    await Promise.all([duplicate, changed]);
    expect(calls).toEqual(['compact', 'settings']);
  });
  it('keeps request identity when resizing equally sized searches at a completion boundary', async () => {
    const calls: string[] = [];
    let release!: () => void;
    const sync = createLayoutSync(async value => {
      calls.push(value.requestId!);
      if (calls.length === 1) await new Promise<void>(resolve => { release = resolve; });
    });
    const first = sync({ layout: 'results', rows: 1, requestId: 'old' });
    release();
    await Promise.resolve();
    const latest = sync({ layout: 'results', rows: 1, requestId: 'new' });
    await Promise.all([first, latest]);
    expect(calls).toEqual(['old', 'new']);
  });
});

describe('result menu positioning', () => {
  it('anchors below a row when the menu fits', () => {
    expect(menuPosition({ right: 890, top: 170, bottom: 197 }, { width: 250, height: 178 }, { width: 918, height: 566 })).toEqual({ left: 640, top: 202 });
  });
  it('flips above a lower row and stays within a compact viewport', () => {
    expect(menuPosition({ right: 890, top: 500, bottom: 527 }, { width: 250, height: 178 }, { width: 918, height: 566 })).toEqual({ left: 640, top: 317 });
    expect(menuPosition({ right: 339, top: 170, bottom: 197 }, { width: 248, height: 178 }, { width: 360, height: 266 })).toEqual({ left: 91, top: 8 });
  });
});

describe('Chinese native feedback', () => {
  it('maps known dependency and action messages without exposing untranslated errors', () => {
    expect(localizeMessage(new Error('Clipboard is unavailable.'))).toContain('剪贴板');
    expect(localizeMessage('Choose Everything.exe to enable file search.')).toBe('文件搜索引擎暂不可用，请在设置中重新启动。');
    expect(localizeMessage('This action requires the desktop app. Preview mode cannot run native actions.')).toContain('桌面应用');
    expect(localizeMessage('unrecognized native fault')).toBe('操作失败，请重试或在设置中查看诊断。');
    expect(localizeMessage('Unknown failure at C:\\项目\\文件.txt')).toBe('操作失败，请重试或在设置中查看诊断。');
    expect(localizeMessage('Unable to open item C:\\文档\\报告.pdf')).toBe('无法打开或定位此项目，请检查文件是否存在。');
    expect(localizeMessage('Unable to open the browser.')).toBe('无法打开必应搜索，请检查默认浏览器后重试。');
  });
  it('keeps existing Chinese feedback and localizes provider states', () => {
    expect(localizeMessage('无法读取本地文件。')).toBe('无法读取本地文件。');
    expect(providerMessage({ id: 'everything', status: 'indexLoading', message: 'File index is loading.' })).toContain('索引');
    expect(providerMessage({ id: 'apps', status: 'ready', message: '7 applications available.' })).toBe('应用搜索已就绪。');
  });
  it('describes bundled engine recovery without requiring a separate client', () => {
    expect(localizeMessage('Everything is not running.')).toBe('文件搜索引擎未运行，可在设置中重新启动。');
    expect(providerMessage({ id: 'everything', status: 'ready', message: 'Everything is ready.' })).toBe('内置 Everything 已就绪。');
    expect(providerMessage({ id: 'everything', status: 'notConfigured', message: '' })).toBe('文件搜索引擎暂不可用，请在设置中重新启动。');
    expect(providerMessage({ id: 'everything', status: 'notRunning', message: '' })).toBe('文件搜索引擎未运行，可在设置中重新启动。');
  });
});
