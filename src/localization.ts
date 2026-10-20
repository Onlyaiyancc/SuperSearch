import type { Provider } from './types';

export function localizeMessage(value: unknown) {
  const raw = value instanceof Error ? value.message : typeof value === 'object' && value && 'message' in value ? String(value.message) : String(value);
  if (!raw) return '';
  // 只转换系统反馈；结果名称、文件路径及诊断原文不经过此适配器。
  if (/^[\u3400-\u9fff]/u.test(raw.trim())) return raw;
  const patterns: [RegExp, string][] = [
    [/file search status is being checked/i, '正在检查搜索引擎状态…'],
    [/fast indexing service stopped/i, '高速索引服务已停止，请重新启动搜索引擎以回退到目录扫描。'],
    [/NTFS\/USN.*active/i, 'NTFS/USN 高速索引已启用。'],
    [/NTFS\/USN.*loading/i, 'NTFS/USN 高速索引正在加载。'],
    [/folder fallback.*active/i, '当前使用目录扫描。安装新版安装包并完成管理员授权，即可启用 NTFS/USN 高速索引。'],
    [/preview|requires the desktop app/i, '预览模式仅展示样例，此操作需要桌面应用。'],
    [/web search|open the browser/i, '无法打开必应搜索，请检查默认浏览器后重试。'],
    [/shortcut.*unavailable|shortcut.*invalid|invalid.*shortcut/i, '快捷键不可用，请在设置中更换。'],
    [/shortcut.*available/i, '全局快捷键已就绪。'],
    [/(choose|select).*Everything\.exe|Everything.*version.*not supported/i, '文件搜索引擎暂不可用，请在设置中重新启动。'],
    [/Everything.*not running|Start Everything|bundled file search.*stopped/i, '文件搜索引擎未运行，可在设置中重新启动。'],
    [/first-time.*indexing.*several minutes/i, '文件索引正在加载，首次建立索引可能需要几分钟。'],
    [/index.*loading/i, '文件索引正在加载。'],
    [/search.*too long|search.*timed out|IPC.*timed out/i, '文件搜索超时，应用搜索仍可使用。'],
    [/Everything.*ready|bundled file search.*ready/i, '内置 Everything 已就绪。'],
    [/Everything|file search/i, '文件搜索引擎暂不可用，请在设置中重新启动。'],
    [/clipboard|copy.*path/i, '无法复制路径，请检查剪贴板后重试。'],
    [/application refresh failed|discover.*limit|enumerate.*application/i, '应用列表刷新失败，已保留现有应用；可在设置中重试。'],
    [/refreshing applications/i, '正在刷新应用列表。'],
    [/application.*(unavailable|no longer available)/i, '此应用已不可用，请刷新应用列表。'],
    [/file.*no longer available|unable to (locate|open|reveal).*item/i, '无法打开或定位此项目，请检查文件是否存在。'],
    [/absolute directory|invalid.*(directory|path)/i, '请输入有效的本地绝对路径。'],
    [/startup.*rollback failed/i, '启动设置更新及恢复失败，请重新加载设置后重试。'],
    [/startup/i, '无法更新开机启动设置，请重试。'],
    [/settings.*(version|changed|conflict)|version.*settings/i, '设置已变更，请重新打开设置后保存。'],
    [/storage|database/i, '本地数据读写失败，请重试或查看诊断。'],
    [/select.*(executable|\.exe|\.lnk)|local application file/i, '请选择本地 .exe 应用或 .lnk 快捷方式。'],
    [/picker|selected file/i, '无法读取所选文件，请重新选择。'],
    [/invalid.*(query|expression)|query.*invalid/i, '搜索表达式无效，请检查筛选条件。'],
  ];
  return patterns.find(([pattern]) => pattern.test(raw))?.[1] ?? '操作失败，请重试或在设置中查看诊断。';
}

export function providerMessage(provider: Provider) {
  if (provider.status === 'ready') return provider.id === 'apps' ? '应用搜索已就绪。' : provider.id === 'shortcut' ? '全局快捷键已就绪。' : provider.message ? localizeMessage(provider.message) : '内置 Everything 已就绪。';
  if (provider.message) return localizeMessage(provider.message);
  if (provider.id === 'everything' && (provider.status === 'notConfigured' || provider.status === 'unavailable')) return '文件搜索引擎暂不可用，请在设置中重新启动。';
  if (provider.id === 'everything' && provider.status === 'notRunning') return '文件搜索引擎未运行，可在设置中重新启动。';
  const labels: Record<Provider['status'], string> = { ready: '搜索已就绪。', loading: '正在加载搜索服务。', indexLoading: '文件索引正在加载。', notRunning: '搜索服务未运行。', notConfigured: '请在设置中检查搜索服务。', unavailable: '搜索服务暂不可用，请在设置中检查。' };
  return labels[provider.status];
}
