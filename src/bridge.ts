import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Settings, ResultItem, SearchSnapshot, Scope, Provider } from './types';
import { hasDirectoryQuery } from './search-state';

export const preview = typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window);
const previewProviders: Provider[] = [
  { id: 'apps', status: 'unavailable', message: '预览仅展示应用样例，桌面应用支持真实应用发现。' },
  { id: 'everything', status: 'unavailable', message: '预览模式未连接内置文件搜索引擎。' },
];
let previewSettings: Settings = {
  settingsVersion: 1, shortcut: 'Alt+Space', autostart: false, recordHistory: true,
  includeDesktop: true, hideHidden: true, hideSystem: true, includeDirectories: [],
  excludeDirectories: [], everythingPath: null, autoStartEverything: false, theme: 'light',
};
const samples: ResultItem[] = [
  { entityId: 'sample-silver-design', title: '设计方案.fig', subtitle: 'D:\\预览样例\\工作\\品牌设计', kind: 'file', pinned: false },
  { entityId: 'sample-silver-assets', title: '设计素材', subtitle: 'D:\\预览样例\\工作', kind: 'folder', pinned: false },
  { entityId: 'sample-silver-guide', title: '设计规范.pdf', subtitle: 'D:\\预览样例\\工作\\文档', kind: 'file', pinned: false },
  { entityId: 'sample-silver-notes', title: '设计记录.docx', subtitle: 'D:\\预览样例\\工作\\文档', kind: 'file', pinned: false },
  { entityId: 'sample-vscode', title: 'Visual Studio Code', subtitle: '预览样例 · 应用', kind: 'application', pinned: true },
  { entityId: 'sample-explorer', title: 'File Explorer', subtitle: '预览样例 · 应用', kind: 'application', pinned: true },
  { entityId: 'sample-terminal', title: 'Terminal', subtitle: '预览样例 · 应用', kind: 'application', pinned: false },
  { entityId: 'sample-project', title: 'Project notes.md', subtitle: '预览样例 · 文档', kind: 'file', pinned: false },
  { entityId: 'sample-design', title: 'Design resources', subtitle: '预览样例 · 文档', kind: 'folder', pinned: false },
].map(item => ({ ...item, resultId: item.entityId, kind: item.kind as ResultItem['kind'], matchTier: 0, available: true, actions: ['open', 'reveal', 'copyPath'] }));

export function previewSnapshot(requestId: string, rawQuery: string, scope: Scope): SearchSnapshot {
  // 保留专用样例前缀供视觉检查；实际磁盘路径始终返回未连接提示。
  if (rawQuery === '示例项目/预算') {
    const items: ResultItem[] = [
      { title: '2026 项目预算.xlsx', kind: 'file', directory: 'D:\\预览样例\\工作\\项目' },
      { title: '预算说明.pdf', kind: 'file', directory: 'D:\\预览样例\\工作\\项目' },
      { title: '预算归档', kind: 'folder', directory: 'E:\\预览样例\\资料\\项目' },
    ].filter(item => scope === 'all' || scope === 'files' && item.kind === 'file' || scope === 'folders' && item.kind === 'folder').map((item, index) => ({
      resultId: `demo-budget-${index}`, entityId: `demo-budget-${index}`, kind: item.kind as ResultItem['kind'], title: item.title,
      subtitle: item.directory, path: `${item.directory}\\${item.title}`, matchTier: 0, pinned: false, available: true, actions: ['open', 'reveal', 'copyPath'],
    }));
    return { requestId, snapshotRevision: 1, scope, complete: true, items, directory: '\\示例项目', terms: ['预算'], folderCompletions: [], providers: previewProviders, truncated: false, hasMore: false };
  }
  if (rawQuery.startsWith('示例项目/')) {
    const remainder = rawQuery.slice('示例项目/'.length);
    const files = remainder.startsWith('报告/');
    const term = files ? remainder.slice('报告/'.length).toLocaleLowerCase() : '';
    const items: ResultItem[] = files && scope !== 'apps' && scope !== 'folders' ? ['年度预算报告.pdf', '预算明细.xlsx', '会议记录.md', '报告草稿.docx', 'Report notes.txt'].filter(title => title.toLocaleLowerCase().includes(term)).map((title, index) => ({
      resultId: `demo-file-${index}`, entityId: `demo-file-${index}`, kind: 'file', title, subtitle: 'D:\\预览样例\\示例项目\\报告', path: `D:\\预览样例\\示例项目\\报告\\${title}`,
      matchTier: 0, pinned: false, available: true, actions: ['open', 'reveal', 'copyPath'],
    })) : [];
    const folderCompletions = !files && scope !== 'apps' ? ['报告', '报表'].filter(title => title.startsWith(remainder)).map(title => ({ title, path: `D:\\预览样例\\示例项目\\${title}`, query: `示例项目/${title}/` })) : [];
    return { requestId, snapshotRevision: 1, scope, complete: true, items, directory: files ? 'D:\\预览样例\\示例项目\\报告' : '\\示例项目', terms: files && term ? [term] : [], folderCompletions, providers: previewProviders, truncated: false, hasMore: false };
  }
  if (hasDirectoryQuery(rawQuery)) throw new Error('预览不读取本地文件夹，请在桌面程序中使用“文件夹/文件”。');
  const query = rawQuery.toLocaleLowerCase().trim();
  const items = samples.filter(item => (!query || item.title.toLocaleLowerCase().includes(query)) &&
    (scope === 'all' || item.kind === ({ apps: 'application', files: 'file', folders: 'folder' } as Record<string, string>)[scope]));
  return { requestId, snapshotRevision: 1, scope, complete: true, items, providers: previewProviders, truncated: false, hasMore: false };
}

// 预览仅展示样例，绝不调用系统启动、路径或文件选择操作。
export async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!preview) return invoke<T>(command, args);
  let result: unknown;
  switch (command) {
    case 'start_search': result = previewSnapshot(args.requestId as string, args.rawQuery as string, args.scope as Scope); break;
    case 'get_settings': result = { ...previewSettings }; break;
    case 'get_provider_status': result = previewProviders; break;
    case 'update_settings':
      previewSettings = { ...previewSettings, ...(args.patch as Partial<Settings>), settingsVersion: previewSettings.settingsVersion + 1 };
      result = { ...previewSettings }; break;
    case 'set_pin': {
      const item = samples.find(item => item.resultId === args.resultId);
      if (item) item.pinned = !!args.enabled;
      break;
    }
    case 'remove_pin': {
      const item = samples.find(item => item.entityId === args.pinId);
      if (item) item.pinned = false;
      break;
    }
    case 'cancel_search': case 'hide_window': case 'sync_window_layout': break;
    case 'get_diagnostics': result = { mode: 'preview', nativeConnection: false, queryLogging: false, engine: { mode: 'bundled', status: 'unavailable', message: previewProviders[1].message } }; break;
    default: throw new Error('预览模式仅展示样例，此操作需要桌面应用。');
  }
  return result as T;
}

export async function subscribe<T>(event: string, handler: (value: T) => void) {
  if (preview) return () => {};
  return listen<T>(event, message => handler(message.payload));
}
