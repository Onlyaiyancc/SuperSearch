import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Settings, ResultItem, SearchSnapshot, Scope, Provider } from './types';
import { hasDirectoryQuery } from './search-state';
import type { Reminder } from './reminders';
import type { QrLinkResult, QrScanResult } from './qr-scan';

export const preview = typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window);
const previewProviders: Provider[] = [
  { id: 'apps', status: 'unavailable', message: '预览仅展示应用样例，桌面应用支持真实应用发现。' },
  { id: 'everything', status: 'unavailable', message: '预览模式未连接内置文件搜索引擎。' },
];
let previewSettings: Settings = {
  settingsVersion: 1, shortcut: 'Alt+Space', autostart: false, recordHistory: true,
  includeDesktop: true, hideHidden: true, hideSystem: true, includeDirectories: [],
  excludeDirectories: [], everythingPath: null, autoStartEverything: false, theme: 'light', windowPlacement: 'remember', translationEnabled: false, filePreviewEnabled: false, remindersEnabled: false, conversionEnabled: false, qrScanEnabled: false, qrScanShortcut: '',
};
let previewQrId = 0;
let previewQrSession: QrScanResult | null = null;
let previewQrLinks: Record<string, QrLinkResult> = {};
let previewReminders: Reminder[] = [];
let previewReminderId = 0;
const samples: ResultItem[] = [
  { entityId: 'sample-silver-design', title: '设计方案.fig', subtitle: 'D:\\预览样例\\工作\\品牌设计', kind: 'file', pinned: false },
  { entityId: 'sample-silver-assets', title: '设计素材', subtitle: 'D:\\预览样例\\工作', kind: 'folder', pinned: false },
  { entityId: 'sample-silver-guide', title: '设计规范.pdf', subtitle: 'D:\\预览样例\\工作\\文档', kind: 'file', pinned: false },
  { entityId: 'sample-silver-notes', title: '设计记录.docx', subtitle: 'D:\\预览样例\\工作\\文档', kind: 'file', pinned: false },
  { entityId: 'sample-vscode', title: 'Visual Studio Code', subtitle: '预览样例 · 应用', kind: 'application', pinned: true },
  { entityId: 'sample-explorer', title: 'File Explorer', subtitle: '预览样例 · 应用', kind: 'application', pinned: true },
  { entityId: 'sample-terminal', title: 'Terminal', subtitle: '预览样例 · 应用', kind: 'application', pinned: false },
  { entityId: 'sample-project', title: 'Project notes.md', subtitle: '预览样例 · 文档', kind: 'file', pinned: false },
  { entityId: 'sample-image', title: 'Preview image.png', subtitle: '预览样例 · 图片', kind: 'file', pinned: false },
  { entityId: 'sample-pdf', title: 'Preview document.pdf', subtitle: '预览样例 · PDF', kind: 'file', pinned: false },
  { entityId: 'sample-xlsx', title: 'Preview workbook.xlsx', subtitle: '预览样例 · 工作簿', kind: 'file', pinned: false },
  { entityId: 'sample-docx', title: 'Preview letter.docx', subtitle: '预览样例 · 文档', kind: 'file', pinned: false },
  { entityId: 'sample-pptx', title: 'Preview slides.pptx', subtitle: '预览样例 · 演示文稿', kind: 'file', pinned: false },
  { entityId: 'sample-log', title: 'Preview log.log', subtitle: '预览样例 · 分段文本', kind: 'file', pinned: false },
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
    case 'prepare_qr_scan':
      if (!previewSettings.qrScanEnabled) throw new Error('QR disabled');
      previewQrSession = { sessionId: `preview-qr-${++previewQrId}`, items: [], limited: false };
      previewQrLinks = {};
      result = { sessionId: previewQrSession.sessionId }; break;
    case 'scan_qr_screen':
      if (!previewSettings.qrScanEnabled) throw new Error('QR disabled');
      if (!previewQrSession || args.sessionId !== previewQrSession.sessionId) throw new Error('QR stale session');
      // 浏览器固定展示合成内容，不读取屏幕或网络，也不调用剪贴板与系统启动。
      previewQrSession.items = [
        { id: 'preview-qr-url', text: 'https://example.com/preview/屏幕扫码', domain: 'example.com', thumbnail: '' },
        { id: 'preview-qr-text', text: '屏幕扫码合成样例\n此内容并非来自屏幕。', domain: null, thumbnail: '' },
      ];
      // 屏幕文字网址样例与二维码内容独立；不通过解析二维码生成这一组结果。
      previewQrSession.webLinks = [
        { id: 'preview-screen-product', url: 'https://screen.example.com/product', domain: 'screen.example.com' },
        { id: 'preview-screen-help', url: 'https://screen.example.org/help', domain: 'screen.example.org' },
      ];
      previewQrSession.webLinksLimited = false;
      previewQrSession.webLinkWarning = '';
      result = structuredClone(previewQrSession); break;
    case 'screen_web_link_action': {
      if (!previewSettings.qrScanEnabled) throw new Error('QR disabled');
      const link = previewQrSession?.webLinks?.find(link => link.id === args.linkId);
      if (!link || previewQrSession?.sessionId !== args.sessionId) throw new Error('Screen link stale session');
      if (args.action !== 'copy' && args.action !== 'open') throw new Error('Screen link invalid action');
      break;
    }
    case 'extract_qr_links': {
      if (!previewSettings.qrScanEnabled) throw new Error('QR disabled');
      const item = previewQrSession?.items.find(item => item.id === args.resultId);
      if (!item || previewQrSession?.sessionId !== args.sessionId) throw new Error('QR stale session');
      // 固定合成凭据只用于界面预览，不实现或调用真实网址提取与系统动作。
      previewQrLinks[item.id] ??= { limited: false, items: item.id === 'preview-qr-url'
        ? [{ id: 'preview-web', url: item.text, domain: 'example.com' }]
        : [{ id: 'preview-product', url: 'https://example.com/product', domain: 'example.com' }, { id: 'preview-help', url: 'https://example.org/help', domain: 'example.org' }] };
      result = structuredClone(previewQrLinks[item.id]); break;
    }
    case 'close_qr_scan':
      if (previewQrSession?.sessionId === args.sessionId) { previewQrSession = null; previewQrLinks = {}; }
      break;
    case 'qr_scan_action': {
      if (!previewSettings.qrScanEnabled) throw new Error('QR disabled');
      const item = previewQrSession?.items.find(item => item.id === args.resultId);
      if (!item || previewQrSession?.sessionId !== args.sessionId) throw new Error('QR stale session');
      if (args.linkId !== undefined) {
        if (!previewQrLinks[item.id]?.items.some(link => link.id === args.linkId)) throw new Error('QR stale link');
        if (args.action !== 'copy' && args.action !== 'open') throw new Error('QR invalid action');
      } else if (args.action !== 'copy' && (args.action !== 'open' || !item.domain)) throw new Error('QR invalid action');
      break;
    }
    case 'list_conversions': result = []; break;
    case 'choose_conversion_files':
      // 合成凭据不含真实路径，选择样例不会打开系统文件选择器。
      result = [{ id: 'preview-conversion-image', name: '图片转换预览样例.png', extension: 'png', bytes: 2048 }]; break;
    case 'get_conversion_components':
      // 浏览器只展示缺失组件样例，绝不下载、执行或写入本地文件。
      result = [ ['office', 'Office 文档'], ['pandoc', '结构化文本'], ['pdf', 'PDF 工具包'], ['ocr', '文字识别'] ].map(([id, name]) => ({
        id, name: `${name}（预览样例）`, description: '请在桌面应用中安装并使用本地组件。', status: 'missing', version: '', downloadBytes: 0, installedBytes: 0, source: '', license: '', error: '', canInstall: false, installationSource: 'unavailable',
      })); break;
    case 'lookup_translation':
      if (!previewSettings.translationEnabled) throw new Error('Translation disabled');
      // 浏览器仅提供明确标注的合成样例，不把输入发送至在线服务。
      if (args.mode === 'dictionary') {
        result = { text: String(args.text), source: 'Free Dictionary API', phonetic: '/həˈləʊ/', meanings: [{ partOfSpeech: '感叹词', definition: 'Used as a greeting or to begin a conversation. (Preview sample)', example: 'Hello, how are you?' }] };
      } else {
        result = { text: args.direction === 'zh-en' ? 'Development plan (preview sample)' : '你好（预览样例）', source: 'MyMemory', phonetic: '', meanings: [] };
      }
      break;
    case 'copy_translation':
      if (!previewSettings.translationEnabled) throw new Error('Translation disabled');
      break;
    case 'preview_file': {
      if (!previewSettings.filePreviewEnabled) throw new Error('Preview disabled');
      const item = samples.find(item => item.resultId === args.resultId && item.kind === 'file');
      if (!item) throw new Error('Preview unsupported format');
      if (item.title.endsWith('.png')) result = { kind: 'image', text: '', mime: 'image/png', data: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg==' };
      else if (item.title.endsWith('.md')) result = { kind: 'markdown', text: '# 项目笔记\n\n这是**预览样例**，不读取本地文件。\n\n- 图片与文本预览\n- Esc 返回搜索', mime: '', data: '' };
      else if (item.title.endsWith('.pdf')) result = { kind: 'pdf', text: '', mime: 'image/png', data: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg==', pageCount: 3, pageIndex: Number(args.page ?? 0) };
      else if (/\.(docx|xlsx|pptx)$/u.test(item.title)) result = { kind: 'office', text: '', mime: '', data: '', sections: item.title.endsWith('.xlsx') ? [{ title: '预算', text: '', rows: [['项目', '金额'], ['开发', '1200'], ['测试', '800']] }, { title: '进度', text: '', rows: [['阶段', '状态'], ['预览扩展', '已完成']] }] : [{ title: item.title.endsWith('.pptx') ? '幻灯片 1' : '文档正文', text: '这是 Office 内容预览样例。\n\n仅展示文字和表格，不完整还原原排版。', rows: [] }] };
      else if (item.title.endsWith('.log')) result = { kind: 'text', text: `第 ${Number(args.offset ?? 0) / 1048576 + 1} 段预览样例\n\n大文本按段读取，不会一次性加载整个文件。`, mime: '', data: '', textOffset: Number(args.offset ?? 0), nextOffset: Number(args.offset ?? 0) < 2097152 ? Number(args.offset ?? 0) + 1048576 : null, totalBytes: 3145728 };
      else throw new Error('Preview unsupported format');
      break;
    }
    case 'list_reminders':
      if (!previewSettings.remindersEnabled) throw new Error('Reminders disabled');
      previewReminders = previewReminders.map(item => item.dueAt <= Date.now() ? { ...item, completed: true } : item);
      result = previewReminders.map(item => ({ ...item })); break;
    case 'create_reminder': {
      if (!previewSettings.remindersEnabled) throw new Error('Reminders disabled');
      const seconds = Number(args.seconds), title = String(args.title ?? '').trim();
      if (!Number.isSafeInteger(seconds) || seconds < 1 || seconds > 604800) throw new Error('Reminder invalid seconds');
      if (!title || Array.from(title).length > 100 || /[\u0000-\u001f\u007f]/u.test(title)) throw new Error('Reminder invalid title');
      if (previewReminders.filter(item => !item.completed).length >= 20) throw new Error('Reminder capacity');
      const item = { id: `preview-${++previewReminderId}`, title, dueAt: Date.now() + seconds * 1000, completed: false };
      previewReminders = [item, ...previewReminders].slice(0, 40);
      result = { ...item }; break;
    }
    case 'cancel_reminder':
      if (!previewSettings.remindersEnabled) throw new Error('Reminders disabled');
      previewReminders = previewReminders.filter(item => item.id !== args.id); break;
    case 'start_search': result = previewSnapshot(args.requestId as string, args.rawQuery as string, args.scope as Scope); break;
    case 'get_settings': result = { ...previewSettings }; break;
    case 'get_provider_status': result = previewProviders; break;
    case 'update_settings':
      previewSettings = { ...previewSettings, ...(args.patch as Partial<Settings>), settingsVersion: previewSettings.settingsVersion + 1 };
      if (!previewSettings.remindersEnabled) previewReminders = previewReminders.filter(item => item.completed);
      if (!previewSettings.qrScanEnabled) { previewQrSession = null; previewQrLinks = {}; }
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
    case 'cancel_search': case 'hide_window': case 'sync_window_layout': case 'close_file_preview': case 'release_conversion_sources': break;
    case 'get_diagnostics': result = { mode: 'preview', nativeConnection: false, queryLogging: false, engine: { mode: 'bundled', status: 'unavailable', message: previewProviders[1].message } }; break;
    default: throw new Error('预览模式仅展示样例，此操作需要桌面应用。');
  }
  return result as T;
}

export async function subscribe<T>(event: string, handler: (value: T) => void) {
  if (preview) return () => {};
  return listen<T>(event, message => handler(message.payload));
}
