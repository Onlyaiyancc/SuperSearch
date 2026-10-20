import { describe, expect, it } from 'vitest';
import { call, previewSnapshot } from '../src/bridge';
import type { Settings } from '../src/types';

describe('browser preview boundary', () => {
  it('defaults to remembered placement and preserves a cursor choice through other edits', async () => {
    const original = await call<Settings>('get_settings');
    expect(original.windowPlacement).toBe('remember');
    try {
      await call('update_settings', { patch: { windowPlacement: 'cursor' } });
      const saved = await call<Settings>('update_settings', { patch: { theme: 'dark' } });
      expect(saved.windowPlacement).toBe('cursor');
    } finally {
      await call('update_settings', { patch: original });
    }
  });
  it('cannot execute a native action or choose an executable', async () => {
    await expect(call('execute_action', { action: 'open', resultId: 'sample-vscode' })).rejects.toThrow('需要桌面应用');
    await expect(call('add_app')).rejects.toThrow('需要桌面应用');
    await expect(call('start_everything')).rejects.toThrow('需要桌面应用');
    await expect(call('search_web', { rawQuery: '年度报告' })).rejects.toThrow('需要桌面应用');
  });
  it('labels all sample results and never reports a native provider as ready', () => {
    const snapshot = previewSnapshot('q-2', '', 'all');
    expect(snapshot.items.every(item => item.subtitle.includes('预览样例'))).toBe(true);
    expect(snapshot.providers.every(provider => provider.status !== 'ready')).toBe(true);
  });
  it('applies scope and text filters to sample data', () => {
    expect(previewSnapshot('q-3', 'project', 'files').items.map(item => item.title)).toEqual(['Project notes.md']);
    expect(previewSnapshot('q-4', 'project', 'apps').items).toEqual([]);
  });
  it('does not substitute unrelated sample files for a real directory query', () => {
    expect(() => previewSnapshot('folder-query', 'D:\\项目 报告', 'all')).toThrow('预览不读取本地文件夹');
    expect(() => previewSnapshot('folder-query', '项目/报告', 'all')).toThrow('预览不读取本地文件夹');
  });
  it('offers the explicit budget visual fixture while preserving preview boundaries and scope', async () => {
    const snapshot = previewSnapshot('budget-query', '示例项目/预算', 'all');
    expect(snapshot.items.map(item => item.title)).toEqual(['2026 项目预算.xlsx', '预算说明.pdf', '预算归档']);
    expect(snapshot.items.every(item => item.path?.includes('预览样例'))).toBe(true);
    expect(snapshot.providers.every(provider => provider.status !== 'ready')).toBe(true);
    expect(previewSnapshot('budget-files', '示例项目/预算', 'files').items).toHaveLength(2);
    expect(previewSnapshot('budget-folders', '示例项目/预算', 'folders').items.map(item => item.title)).toEqual(['预算归档']);
    expect(previewSnapshot('budget-apps', '示例项目/预算', 'apps').items).toEqual([]);
    expect(previewSnapshot('report-files', '示例项目/报告/', 'all').items).toHaveLength(5);
    await expect(call('execute_action', { action: 'open', resultId: snapshot.items[0].resultId })).rejects.toThrow('需要桌面应用');
  });
  it('treats native window resize as a harmless preview no-op', async () => {
    await expect(call('sync_window_layout', { layout: 'compact', rows: 0 })).resolves.toBeUndefined();
    await expect(call('sync_window_layout', { layout: 'results', rows: 2, requestId: 'sample' })).resolves.toBeUndefined();
  });
  it('identifies the bundled engine in diagnostics without claiming native connectivity', async () => {
    await expect(call('get_diagnostics')).resolves.toMatchObject({ mode: 'preview', nativeConnection: false, engine: { mode: 'bundled', status: 'unavailable' } });
  });
  it('round-trips legacy engine settings while changing an unrelated preference', async () => {
    const original = await call<Settings>('get_settings');
    const legacy = { everythingPath: 'C:\\Legacy\\Everything.exe', autoStartEverything: true };
    try {
      await call('update_settings', { patch: legacy });
      const draft = await call<Settings>('get_settings');
      const { settingsVersion, ...patch } = { ...draft, recordHistory: false };
      const saved = await call('update_settings', { settingsVersion, patch });
      expect(saved).toMatchObject({ ...legacy, recordHistory: false });
    } finally {
      const { settingsVersion, ...patch } = original;
      await call('update_settings', { patch });
    }
  });
});
