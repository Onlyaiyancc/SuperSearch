import { Marked } from 'marked';

export type OfficeSection = { title: string; text: string; rows: string[][] };
export type FilePreview = { kind: string; text: string; mime: string; data: string; pageCount?: number; pageIndex?: number; textOffset?: number; nextOffset?: number | null; totalBytes?: number; truncated?: boolean; sections?: OfficeSection[] };
export type PreviewPosition = { page?: number; offset?: number };

export function formatPreviewBytes(bytes?: number) {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return '';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1048576).toFixed(1)} MiB`;
}

export function parsePreviewPage(value: string, total: number) {
  if (!/^\d+$/u.test(value.trim())) return null;
  const page = Number(value.trim());
  return Number.isSafeInteger(page) && page >= 1 && page <= total ? page - 1 : null;
}

export function previewZoomStep(value: number, direction: -1 | 1) {
  const stops = [0.1, 0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4];
  return direction > 0 ? stops.find(stop => stop > value + 0.001) ?? 4 : stops.slice().reverse().find(stop => stop < value - 0.001) ?? 0.1;
}

export function previewReadingKey(event: { key: string; ctrlKey?: boolean; altKey?: boolean; metaKey?: boolean; shiftKey?: boolean; isComposing?: boolean; keyCode?: number; repeat?: boolean; defaultPrevented?: boolean }, context: { kind: string; blocked: boolean; fit: boolean; previous: boolean; next: boolean }) {
  if (context.blocked || event.defaultPrevented || event.isComposing || event.keyCode === 229 || event.repeat || event.altKey || event.metaKey) return null;
  if (event.ctrlKey && ['pdf', 'image'].includes(context.kind)) {
    if (event.key === '+' || event.key === '=') return 'zoomIn';
    if (event.key === '-') return 'zoomOut';
    if (event.key === '0') return 'fit';
  }
  if (event.ctrlKey || event.shiftKey) return null;
  const arrows = context.kind === 'pdf' && context.fit;
  if ((event.key === 'PageUp' || arrows && event.key === 'ArrowLeft') && context.previous) return 'previous';
  if ((event.key === 'PageDown' || arrows && event.key === 'ArrowRight') && context.next) return 'next';
  return null;
}

export function createPreviewSession(read: (position: PreviewPosition) => Promise<FilePreview>, close: () => void, changed: () => void) {
  const state = { content: null as FilePreview | null, loading: false, error: '' };
  let disposed = false;
  return {
    state,
    async load(position: PreviewPosition = {}) {
      if (disposed || state.loading) return false;
      state.loading = true; state.error = ''; changed();
      try {
        const value = await read(position);
        if (disposed) return false;
        state.content = value;
        return true;
      } catch (error) {
        if (!disposed) state.error = previewError(error);
        return false;
      } finally {
        state.loading = false;
        if (!disposed) changed();
      }
    },
    dispose() { if (!disposed) { disposed = true; close(); } },
  };
}

export function imagePreviewHeight(width: number, height: number, availableWidth: number, chromeHeight: number) {
  const displayedHeight = height * Math.min(1, Math.max(1, availableWidth) / Math.max(1, width));
  return Math.min(620, Math.max(220, Math.ceil(chromeHeight + displayedHeight)));
}

export function previewShortcut(event: { key: string; ctrlKey?: boolean; altKey?: boolean; metaKey?: boolean; shiftKey?: boolean; repeat?: boolean }, context: { enabled: boolean; established: boolean; inputFocused: boolean; available: boolean; kind: string; pending: boolean }) {
  return event.key === ' ' && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey && !event.repeat
    && context.enabled && context.established && context.inputFocused && context.available && context.kind === 'file' && !context.pending;
}

const markdown = new Marked({ async: false, renderer: {
  // 原始 HTML 不渲染；链接和图片仅保留文字，不创建任何外部资源入口。
  html() { return ''; },
  link({ tokens }) { return this.parser.parseInline(tokens); },
  image({ text }) { return text.replace(/[&<>"']/g, character => `&#${character.charCodeAt(0)};`); },
} });

export function markdownHtml(text: string): string {
  return markdown.parse(text, { async: false });
}

export function previewError(value: unknown) {
  const raw = String(value);
  if (/disabled/i.test(raw)) return '文件快速预览尚未启用。';
  if (/large|limit|size/i.test(raw)) return '文件超过预览上限：PDF 100 MiB，图片与 Office 50 MiB，文本 1 GiB；解码后的内容也有安全上限。';
  if (/Preview (PDF )?busy/i.test(raw)) return '上一项仍在处理，请稍后重试。';
  if (/PDF/i.test(raw)) return '无法预览此 PDF，它可能已损坏或需要密码，请使用关联应用打开。';
  if (/office|zip|xml|关系|文档|工作表|压缩|实体|解析|内容|条目|外部|格式|单元格/iu.test(raw)) return '无法预览此 Office 文档，文件可能已损坏、包含外部引用或超过内容限制。';
  if (/encoding|utf|binary/i.test(raw)) return '无法预览此文本编码，请使用关联应用打开。';
  if (/unsupported|format|image/i.test(raw)) return '暂不支持此文件格式，请使用关联应用打开。';
  if (/network|device|local|path/i.test(raw)) return '仅支持常规本地文件预览。';
  if (/no longer|current/i.test(raw)) return '搜索结果已更新，请重新选择文件。';
  return '无法读取此文件，它可能已移动或没有读取权限。';
}
