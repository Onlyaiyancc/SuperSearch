export type ConversionSource = { id: string; name: string; extension: string; bytes: number };
export type ConversionDirectory = { id: string; label: string };
export type ConversionOptions = { target: string; quality: number; dpi: number; icoSize: number; pages: string; sheet: number | null; merge: boolean; ocr: boolean; csvDelimiter: string; csvEncoding: string };
export type ConversionStatus = 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';
export type ConversionOutput = { id: string; name: string; path: string };
export type ConversionItem = { sourceId?: string; name: string; status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled'; outputs: ConversionOutput[]; error: string };
export type ConversionJob = { id: string; status: ConversionStatus; target: string; items: ConversionItem[]; completed: number; total: number; options?: ConversionOptions };
export type ConversionComponentId = 'office' | 'pandoc' | 'pdf' | 'ocr';
export type ConversionComponent = { id: ConversionComponentId; name: string; description: string; status: 'missing' | 'downloading' | 'installing' | 'ready' | 'failed'; version: string; downloadBytes: number; installedBytes: number; source: string; license: string; error: string; managed?: boolean; canInstall: boolean; installationSource: 'adjacent' | 'download' | 'unavailable' };

export function parseConversionQuery(query: string) {
  return /^(转换|convert)$/iu.test(query.trim());
}

const images = ['jpg', 'png', 'webp', 'bmp', 'tiff', 'gif', 'ico'];
const writers = ['doc', 'docx', 'odt', 'rtf'];
const sheets = ['xls', 'xlsx', 'ods'];
const slides = ['ppt', 'pptx', 'odp'];
const text = ['md', 'txt', 'html'];
const order = [...images, 'pdf', ...writers, 'txt', 'html', 'md', ...sheets, 'csv', ...slides];
const normalize = (extension: string) => ({ jpeg: 'jpg', tif: 'tiff', htm: 'html', markdown: 'md' })[extension.toLowerCase().replace(/^\./u, '')] ?? extension.toLowerCase().replace(/^\./u, '');

function targets(extension: string) {
  const source = normalize(extension);
  if (images.includes(source)) return [...images, 'pdf', 'txt', 'docx'];
  if (writers.includes(source)) return [...writers, 'pdf', 'txt', 'html', ...(source === 'docx' ? ['md'] : [])];
  if (sheets.includes(source)) return [...sheets, 'csv', 'pdf'];
  if (slides.includes(source)) return [...slides, 'pdf'];
  if (source === 'csv') return ['xlsx', 'ods'];
  if (text.includes(source)) return [...text, 'docx', 'pdf'];
  if (source === 'pdf') return ['png', 'jpg', 'txt', 'docx', 'xlsx', 'csv', 'pptx', 'pdf'];
  return [];
}

export function commonTargets(extensions: string[]) {
  if (!extensions.length) return [];
  const mixedImages = extensions.some(source => images.includes(normalize(source))) && extensions.some(source => !images.includes(normalize(source)));
  return [...new Set(order)].filter(target => extensions.every(source => targets(source).includes(target)) && !(mixedImages && ['txt', 'docx'].includes(target)));
}

export function mergeConversionSources(existing: ConversionSource[], selected: ConversionSource[]) {
  // 只有原生文件凭据能证明身份；不同目录中同名同大小的文件必须分别保留。
  const seen = new Set<string>();
  return [...existing, ...selected].filter(source => {
    if (seen.has(source.id)) return false;
    seen.add(source.id);
    return true;
  });
}

export type ConversionDraft = { sources: ConversionSource[]; directory: ConversionDirectory | null; options: ConversionOptions };
let savedDraft: ConversionDraft | null = null;
let clearingDraft: Promise<void> | null = null;
const consumedInitialSelections = new WeakSet<ConversionSource[]>();

export function pendingConversionDraftClear() { return clearingDraft; }

export function clearConversionDraftSources(ids: string[], release: (ids: string[]) => Promise<unknown>) {
  if (clearingDraft) return clearingDraft;
  const released = new Set(ids);
  const operation = (async () => {
    await release([...released]);
    // 面板可能已经切换；只从最新草稿移除本次已释放的凭据，保留新加入的文件和选项。
    if (savedDraft) savedDraft = { ...savedDraft, sources: savedDraft.sources.filter(source => !released.has(source.id)) };
  })();
  const completion = operation.finally(() => { if (clearingDraft === completion) clearingDraft = null; });
  clearingDraft = completion;
  return completion;
}

export function saveConversionDraft(draft: ConversionDraft) {
  // 草稿仅留在当前进程内；隐藏、设置切换后恢复，不写入磁盘或转换历史。
  savedDraft = { sources: draft.sources.slice(0, 100).map(source => ({ ...source })), directory: draft.directory ? { ...draft.directory } : null, options: { ...defaultConversionOptions(), ...draft.options } };
}

export function loadConversionDraft(initialSources: ConversionSource[] = []): ConversionDraft {
  const draft = savedDraft ? { sources: savedDraft.sources.map(source => ({ ...source })), directory: savedDraft.directory ? { ...savedDraft.directory } : null, options: { ...defaultConversionOptions(), ...savedDraft.options } } : { sources: [], directory: null, options: defaultConversionOptions() };
  // 同一菜单选择在重新挂载时只消费一次；新的主动选择即使复用文件凭据也应允许重新添加。
  if (!consumedInitialSelections.has(initialSources)) {
    consumedInitialSelections.add(initialSources);
    for (const source of initialSources) {
      if (!draft.sources.some(item => item.id === source.id) && draft.sources.length < 100) draft.sources.push({ ...source });
    }
  }
  draft.options = optionsForConversionSources(draft.sources.map(source => source.extension), draft.options);
  return draft;
}

export function defaultConversionOptions(target = 'png'): ConversionOptions {
  return { target, quality: 90, dpi: 150, icoSize: 256, pages: '', sheet: null, merge: false, ocr: false, csvDelimiter: ',', csvEncoding: 'utf-8' };
}

export function optionsForConversionSources(extensions: string[], options: ConversionOptions): ConversionOptions {
  const restored = { ...defaultConversionOptions(), ...options };
  // 清空文件只改变选择，不抹掉用户的目标格式；再次添加兼容文件时继续使用原选项。
  if (!extensions.length) return restored;
  if (restored.target !== 'pdf' || !extensions.every(extension => images.includes(normalize(extension)))) restored.merge = false;
  if (!extensions.some(extension => normalize(extension) === 'pdf' || images.includes(normalize(extension))) || !['txt', 'docx', 'pdf'].includes(restored.target)) restored.ocr = false;
  const available = commonTargets(extensions);
  if (available.includes(restored.target)) return restored;
  return { ...restored, target: available.includes('pdf') ? 'pdf' : available[0] ?? '', merge: false, ocr: false };
}

export function requiredComponents(extensions: string[], options: ConversionOptions): ConversionComponentId[] {
  const required = new Set<ConversionComponentId>();
  for (const extension of extensions) {
    const source = normalize(extension);
    if (source === 'pdf' || images.includes(source) && (options.target === 'pdf' || ['txt', 'docx'].includes(options.target))) required.add('pdf');
    if (writers.includes(source) && options.target !== 'md' || sheets.includes(source) || slides.includes(source) || source === 'csv') required.add('office');
    if (text.includes(source) || source === 'docx' && options.target === 'md') required.add('pandoc');
    if (text.includes(source) && options.target === 'pdf') required.add('office');
    if (options.ocr && ['txt', 'docx', 'pdf'].includes(options.target) && (source === 'pdf' || images.includes(source)) || images.includes(source) && ['txt', 'docx'].includes(options.target)) required.add('ocr');
  }
  return (['office', 'pandoc', 'pdf', 'ocr'] as const).filter(id => required.has(id));
}

export function validateConversionOptions(extensions: string[], options: ConversionOptions) {
  if (!commonTargets(extensions).includes(options.target)) return '这些文件没有共同可用的目标格式，请分别转换。';
  if (![',', ';', '\t'].includes(options.csvDelimiter)) return 'CSV 分隔符请选择逗号、分号或制表符。';
  if (!['utf-8', 'gb18030', 'utf-16le', 'utf-16be'].includes(options.csvEncoding)) return 'CSV 编码请选择 UTF-8、GB18030 或 UTF-16。';
  if (!Number.isInteger(options.quality) || options.quality < 1 || options.quality > 100) return '图片质量应为 1 到 100。';
  if (![150, 300].includes(options.dpi)) return '请选择 150 或 300 DPI。';
  if (![16, 32, 48, 64, 128, 256].includes(options.icoSize)) return '请选择支持的 ICO 图标尺寸。';
  if (options.pages && !options.pages.split(',').every(part => {
    const match = /^(\d+)(?:-(\d+))?$/u.exec(part.trim());
    return match && Number(match[1]) >= 1 && Number(match[2] ?? match[1]) >= Number(match[1]) && Number(match[2] ?? match[1]) <= 500;
  })) return '页码或帧号请填写 1、1-3 或 1-3,5，范围为 1 到 500。';
  if (options.target === 'csv' && extensions.some(extension => sheets.includes(normalize(extension))) && (!Number.isInteger(options.sheet) || Number(options.sheet) < 1 || Number(options.sheet) > 1000)) return '导出 CSV 前请选择工作表序号（从 1 开始）。';
  if (options.target === 'pdf' && extensions.some(extension => normalize(extension) === 'pdf') && !options.ocr) return 'PDF 转可搜索 PDF 需要开启文字识别。';
  if (['txt', 'docx'].includes(options.target) && extensions.some(extension => images.includes(normalize(extension))) && !options.ocr) return '从图片提取可编辑文字需要开启文字识别。';
  if (options.merge && (options.target !== 'pdf' || !extensions.every(extension => images.includes(normalize(extension))))) return '合并 PDF 仅适用于图片批次。';
  return '';
}

export function targetLabel(target: string, extensions: string[] = []) {
  if (extensions.some(extension => normalize(extension) === 'pdf')) {
    if (target === 'docx') return 'Word · 提取可编辑文字';
    if (target === 'xlsx') return 'Excel · 提取表格';
    if (target === 'csv') return 'CSV · 每张表单独文件';
    if (target === 'pptx') return 'PPT · 每页作为图片';
    if (target === 'pdf') return 'PDF · 文字识别后可搜索';
  }
  return ({ jpg: 'JPG', png: 'PNG', webp: 'WebP', bmp: 'BMP', tiff: 'TIFF', gif: 'GIF', ico: 'ICO', pdf: 'PDF', doc: 'Word 97–2003 (.doc)', docx: 'Word (.docx)', odt: 'ODT', rtf: 'RTF', txt: '纯文本 (.txt)', html: 'HTML', md: 'Markdown', xls: 'Excel 97–2003 (.xls)', xlsx: 'Excel (.xlsx)', ods: 'ODS', csv: 'CSV', ppt: 'PowerPoint 97–2003 (.ppt)', pptx: 'PowerPoint (.pptx)', odp: 'ODP' })[target] ?? target.toUpperCase();
}

export function conversionError(value: unknown) {
  const raw = value instanceof Error ? value.message : String(value);
  const messages: [RegExp, string][] = [
    [/disabled/iu, '请先在设置 → 扩展中启用“格式转换”。'],
    [/conversion.*job.*(unavailable|expired|not found)/iu, '原转换任务已失效，请重新选择文件并开始转换。'],
    [/conversion.*retry/iu, '仅可重试已结束任务中的失败项；合并 PDF 请使用“重试合并任务”。'],
    [/csv.*(encoding|decode)|csv 编码/iu, 'CSV 编码无法正确读取，请更换编码后重新预览。'],
    [/csv.*delimiter|csv 分隔符/iu, 'CSV 分隔符无效，请选择逗号、分号或制表符。'],
    [/csv.*(limit|too large)/iu, 'CSV 导入和预览仅支持不超过 32 MiB 的文件。'],
    [/csv.*(invalid|malformed|parse|quote)/iu, 'CSV 内容无法解析，请检查分隔符、引号及文件编码。'],
    [/no tables|table.*not found/iu, '未检测到可提取的表格，请检查 PDF 内容。'],
    [/no text|empty text|scan/iu, '没有可提取的文字，扫描件请开启文字识别。'],
    [/component.*package.*not published/iu, '此版本未提供可信组件包，请使用包含该组件的完整发行包。'],
    [/component requires local package/iu, '未找到随附组件包，请保留程序旁的 components 文件夹，或导入与此版本匹配的本地组件包。'],
    [/installer launched/iu, '安装程序已打开，请完成 Windows 安装后刷新组件状态。'],
    [/checksum|sha.?256|digest|signature|component size mismatch/iu, '组件校验失败，请重新下载可信的完整组件包。'],
    [/component download.*HTTP (401|403)\b/iu, '组件下载被服务器拒绝，请检查网络或代理；也可安装随附组件包或导入本地组件包。'],
    [/component download.*HTTP (404|410)\b/iu, '此固定版本的组件下载地址暂不可用，请使用随附组件包或导入匹配的本地组件包。'],
    [/component download.*timeout/iu, '组件下载超时，请检查网络或代理后重试；也可使用随附组件包。'],
    [/component download.*redirect/iu, '组件下载的重定向未通过安全检查，请使用随附组件包或导入匹配的本地组件包。'],
    [/component.*(network|download|http|connect)|untrusted component redirect/iu, '组件下载失败，请检查网络；也可导入本地组件包。'],
    [/component.*(entry|manifest|probe)|zip|archive|package|manifest/iu, '组件包格式或内容无效，请选择此组件的完整官方或项目分发包。'],
    [/component (?:office |pandoc |pdf |ocr )?(missing|not installed)$|missing.*component/iu, '请先安装此转换所需的本地组件。'],
    [/queue|capacity/iu, '转换队列已满，请等待当前任务完成后重试。'],
    [/cancelled|canceled/iu, '操作已取消，原文件保留。'],
    [/timeout|timed out/iu, '处理超时，可减少文件或页数后重试。'],
    [/multiframe|multi.?frame|multiple.*(frames|pages)/iu, '此文件包含多帧或多页，请填写要转换的帧号或页码。'],
    [/sheet/iu, '请选择有效的工作表序号（从 1 开始）。'],
    [/page|frame/iu, '页码或帧号无效，请检查文件内容和范围。'],
    [/limit|too large|too many|exceed|memory/iu, '文件、页数或输出超出处理上限，请缩小批次后重试。'],
    [/source|grant|no longer|not found/iu, '源文件已变更或选择已失效，请重新选择文件。'],
    [/network|download|http|connect/iu, '组件下载失败，请检查网络；也可导入本地组件包。'],
    [/permission|access|denied|directory|disk|space/iu, '无法读写所选位置，请检查权限和剩余空间。'],
    [/busy|in use/iu, '组件正在使用中，请等待任务完成后操作。'],
    [/unsupported|format|target/iu, '此转换方向或文件格式暂不可用，请选择其他格式。'],
    [/preview|desktop|预览|桌面应用/iu, '浏览器预览仅展示样例，请在桌面应用中操作。'],
  ];
  return messages.find(([pattern]) => pattern.test(raw))?.[1] ?? '转换操作失败，请检查文件和组件后重试。';
}

export function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return '—';
  if (bytes < 1024 * 1024) return `${Math.ceil(bytes / 1024)} KiB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
}

export { createLatestRefresh as createConversionRefresh } from './refresh';

export function refreshConversionEvent(event: 'conversions_changed' | 'conversion_components_changed', jobs: { run: () => Promise<void> }, components: { run: () => Promise<void> }, enabled = true) {
  if (event === 'conversion_components_changed' && !enabled) return Promise.resolve();
  return (event === 'conversions_changed' ? jobs : components).run();
}

export function conversionRetryRequest(job: ConversionJob, sourceId: string | null) {
  if (job.status !== 'failed') return null;
  if (job.options?.merge) return sourceId === null ? { jobId: job.id, sourceId: null } : null;
  return sourceId && job.items.some(item => item.sourceId === sourceId && item.status === 'failed') ? { jobId: job.id, sourceId } : null;
}

export type CsvPreviewRequest = { sourceId: string; delimiter: string; encoding: string };
export type CsvPreviewResult = { rows: string[][]; truncated: boolean };
export type CsvPreviewState = { key: string; pending: boolean; completed: number; total: number; results: Record<string, CsvPreviewResult>; error: string };

export function csvPreviewKey(sources: ConversionSource[], delimiter: string, encoding: string) {
  return JSON.stringify([sources.filter(source => normalize(source.extension) === 'csv').map(source => [source.id, source.bytes]), delimiter, encoding]);
}

export function csvPreviewReady(state: CsvPreviewState, sources: ConversionSource[], delimiter: string, encoding: string) {
  const csvSources = sources.filter(source => normalize(source.extension) === 'csv');
  return !csvSources.length || !state.pending && !state.error && state.key === csvPreviewKey(csvSources, delimiter, encoding) && csvSources.every(source => !!state.results[source.id]);
}

export function createCsvPreviewSession(read: (request: CsvPreviewRequest) => Promise<CsvPreviewResult>, changed: (state: CsvPreviewState) => void) {
  let epoch = 0;
  let disposed = false;
  return {
    reset() {
      epoch++;
      if (!disposed) changed({ key: '', pending: false, completed: 0, total: 0, results: {}, error: '' });
    },
    async preview(sources: ConversionSource[], delimiter: string, encoding: string) {
      if (disposed) return;
      const current = ++epoch;
      const csvSources = sources.filter(source => normalize(source.extension) === 'csv');
      const key = csvPreviewKey(csvSources, delimiter, encoding);
      const results: Record<string, CsvPreviewResult> = {};
      let completed = 0;
      changed({ key, pending: true, completed, total: csvSources.length, results: {}, error: '' });
      try {
        const invalid = validateConversionOptions(['csv'], { ...defaultConversionOptions('xlsx'), csvDelimiter: delimiter, csvEncoding: encoding });
        if (invalid) throw new Error(invalid);
        for (const source of csvSources) {
          if (source.bytes > 32 * 1024 * 1024) throw new Error('CSV source limit exceeded');
          const result = await read({ sourceId: source.id, delimiter, encoding });
          if (disposed || current !== epoch) return;
          // 预览只展示有界文本；不把单元格解释成 HTML、公式或应用代码。
          results[source.id] = { rows: result.rows.slice(0, 20).map(row => row.slice(0, 20).map(cell => String(cell).slice(0, 2048))), truncated: result.truncated || result.rows.length > 20 || result.rows.some(row => row.length > 20 || row.some(cell => String(cell).length > 2048)) };
          completed++;
          changed({ key, pending: true, completed, total: csvSources.length, results: { ...results }, error: '' });
        }
        if (!disposed && current === epoch) changed({ key, pending: false, completed, total: csvSources.length, results: { ...results }, error: '' });
      } catch (value) {
        if (!disposed && current === epoch) changed({ key, pending: false, completed, total: csvSources.length, results: { ...results }, error: conversionError(value) });
      }
    },
    dispose() { disposed = true; epoch++; },
  };
}
