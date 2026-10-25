export type QrScanItem = { id: string; text: string; thumbnail: string; domain: string | null };
export type QrScanResult = { sessionId: string; items: QrScanItem[]; limited: boolean; webLinks?: QrWebLink[]; webLinksLimited?: boolean; webLinkWarning?: string };
export type QrWebLink = { id: string; url: string; domain: string };
export type QrLinkResult = { items: QrWebLink[]; limited: boolean };
export type QrScanState = { pending: boolean; acting: boolean; extracting: string | null; links: Record<string, QrLinkResult>; result: QrScanResult | null; selected: number; error: string; notice: string };
type QrScanPort = {
  prepare: () => Promise<{ sessionId: string }>;
  scan: (sessionId: string) => Promise<QrScanResult>;
  close: (sessionId: string) => Promise<unknown>;
  extract?: (sessionId: string, resultId: string) => Promise<QrLinkResult>;
  action: (sessionId: string, resultId: string, action: 'copy' | 'open', linkId?: string) => Promise<unknown>;
  screenAction: (sessionId: string, linkId: string, action: 'copy' | 'open') => Promise<unknown>;
};

export function isQrScanQuery(query: string) { return /^(扫码|scan|qr|扫一扫|扫扫码)$/iu.test(query.trim()); }

export function screenLinkWarning(raw = '') {
  if (!raw) return '';
  if (/missing|local package|not published/i.test(raw)) return '屏幕网址识别需要本地文字识别组件，请使用完整包并保留 components 文件夹后重新扫描。';
  if (/timeout|timed out/i.test(raw)) return '屏幕网址识别超时，请减少屏幕内容后重试。';
  if (/busy|in use/i.test(raw)) return '文字识别组件正在使用中，请稍后重新扫描。';
  return '屏幕网址识别暂不可用；二维码结果仍可使用。请重新扫描。';
}

export function qrScanError(value: unknown) {
  const raw = value instanceof Error ? value.message : String(value);
  const messages: [RegExp, string][] = [
    [/disabled/i, '请先在设置 → 扩展中启用“屏幕扫码”。'],
    [/clipboard/i, '无法复制内容，请检查剪贴板后重试。'],
    [/shortcut.*conflict|conflict.*shortcut/i, '扫码快捷键与搜索快捷键冲突，请使用不同的组合。'],
    [/shortcut/i, '扫码快捷键无法注册，请尝试其他组合。'],
    [/pixel|dimension|buffer|screen.*large|capture.*limit/i, '当前屏幕尺寸超过扫描上限，请切换到较小的显示器。'],
    [/expired|stale|session|cancel/i, '本次扫描已失效，请重新扫描。'],
    [/busy/i, '扫描仍在处理中，请稍候。'],
    [/extract.*link|link.*extract/i, '无法提取网页链接，请重试。'],
    [/open|url/i, '无法打开此链接，请重试或复制内容。'],
    [/content|text.*limit|too long/i, '二维码内容超过支持范围，无法执行此操作。'],
    [/capture|monitor|desktop/i, '无法捕获当前屏幕，请返回普通桌面后重试。'],
  ];
  return messages.find(([pattern]) => pattern.test(raw))?.[1] ?? '屏幕扫码失败，请重新扫描。';
}

export function createQrScanSession(port: QrScanPort, changed: (state: QrScanState) => void) {
  let epoch = 0;
  let sessionId: string | null = null;
  const empty = (): QrScanState => ({ pending: false, acting: false, extracting: null, links: {}, result: null, selected: 0, error: '', notice: '' });
  let state = empty();
  const publish = (patch: Partial<QrScanState>) => { state = { ...state, ...patch }; changed(state); };
  const close = (id: string) => { void port.close(id).catch(() => {}); };
  const visibleCount = () => (state.result?.webLinks?.length ?? 0) + (state.result?.items.length ?? 0);

  function cancel() {
    // 先同步失效，再等待原生关闭；准备阶段迟到的会话由 scan 负责清理。
    epoch++;
    if (sessionId) close(sessionId);
    sessionId = null;
    state = empty(); changed(state);
  }

  async function scan() {
    if (state.pending || state.acting) return;
    cancel();
    const current = epoch;
    publish({ pending: true });
    try {
      const prepared = await port.prepare();
      if (current !== epoch) { close(prepared.sessionId); return; }
      sessionId = prepared.sessionId;
      const result = await port.scan(sessionId);
      if (current !== epoch) return;
      if (result.sessionId !== sessionId) throw new Error('QR stale session');
      publish({ pending: false, result });
    } catch (value) {
      if (current !== epoch) return;
      if (sessionId) close(sessionId);
      sessionId = null;
      publish({ pending: false, error: qrScanError(value) });
    }
  }

  async function extract(index = state.selected) {
    const item = state.result?.items[index];
    if (state.pending || state.acting || !sessionId || !item || !port.extract) return;
    const current = epoch;
    publish({ acting: true, extracting: item.id, selected: index, error: '', notice: '' });
    try {
      const links = await port.extract(sessionId, item.id);
      if (current === epoch) publish({ acting: false, extracting: null, links: { ...state.links, [item.id]: links } });
    } catch (value) {
      if (current === epoch) publish({ acting: false, extracting: null, error: qrScanError(value) });
    }
  }

  async function action(action: 'copy' | 'open', index = state.selected, linkId?: string) {
    const webLinks = state.result?.webLinks ?? [];
    const webLink = webLinks[index];
    const item = state.result?.items[index - webLinks.length];
    if (state.pending || state.acting || !sessionId || (!webLink && !item)) return;
    if (item && !webLink && (linkId !== undefined ? !state.links[item.id]?.items.some(link => link.id === linkId) : action === 'open' && !item.domain)) return;
    const current = epoch;
    publish({ acting: true, selected: index, error: '', notice: '' });
    try {
      // 只提交原生结果凭据，文本与网址始终由原生会话解析。
      if (webLink) await port.screenAction(sessionId, webLink.id, action);
      else if (item) {
        if (linkId !== undefined) await port.action(sessionId, item.id, action, linkId);
        else await port.action(sessionId, item.id, action);
      }
      if (current === epoch) publish({ acting: false, notice: action === 'copy' ? webLink || linkId !== undefined ? '链接已复制到剪贴板' : '已复制到剪贴板' : '已打开链接' });
    } catch (value) {
      if (current === epoch) publish({ acting: false, error: qrScanError(value) });
    }
  }

  return {
    cancel, scan, action, extract,
    async submit() { if (visibleCount()) await action('copy'); else await scan(); },
    select(index: number) { if (!state.acting && Number.isInteger(index) && index >= 0 && index < visibleCount()) publish({ selected: index }); },
    move(direction: number) {
      const length = visibleCount();
      if (length && !state.acting) publish({ selected: (state.selected + direction + length) % length });
    },
  };
}
