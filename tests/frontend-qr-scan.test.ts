import { describe, expect, it, vi } from 'vitest';
import { createQrScanSession, isQrScanQuery, qrScanError, screenLinkWarning, type QrLinkResult, type QrScanResult, type QrScanState } from '../src/qr-scan';
import { call } from '../src/bridge';
import type { Settings } from '../src/types';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (value: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const result: QrScanResult = { sessionId: 'scan-1', limited: false, items: [
  { id: 'url', text: 'https://example.com/中文', thumbnail: '', domain: 'example.com' },
  { id: 'text', text: '原文\n内容', thumbnail: '', domain: null },
] };
function setup(overrides = {}) {
  let state: QrScanState;
  const port = { prepare: vi.fn(async () => ({ sessionId: 'scan-1' })), scan: vi.fn(async () => result), close: vi.fn(async (_id: string) => {}), extract: vi.fn(async (_sessionId: string, _resultId: string): Promise<QrLinkResult> => ({ items: [], limited: false })), action: vi.fn(async (_sessionId: string, _resultId: string, _action: 'copy' | 'open', _linkId?: string) => {}), screenAction: vi.fn(async (_sessionId: string, _linkId: string, _action: 'copy' | 'open') => {}), ...overrides };
  const session = createQrScanSession(port, value => { state = value; });
  return { port, session, state: () => state };
}

describe('QR command routing', () => {
  it.each(['qr', 'QR', '  qR\t', ' 扫码 ', '扫一扫', '扫扫码', 'scan', 'SCAN', '  Scan\t'])('accepts exact commands: %s', query => expect(isQrScanQuery(query)).toBe(true));
  it.each(['qrcode', 'qr.png', 'qr text', 'scan.png', 'scanner.md', 'scan text', '扫码文档', '扫码 二维码', '扫一扫说明', '扫扫码文档', 'C:\\qr', ''])('preserves ordinary search: %s', query => expect(isQrScanQuery(query)).toBe(false));
});

describe('URLs visibly printed on the captured screen', () => {
  const webLinks = [
    { id: 'screen-first', url: 'https://screen.example/visible', domain: 'screen.example' },
    { id: 'screen-second', url: 'https://another.example/help', domain: 'another.example' },
  ];
  const combined: QrScanResult = { ...result, webLinks, webLinksLimited: false, webLinkWarning: '' };

  it('maps OCR warnings to actionable Chinese without exposing native detail', () => {
    expect(screenLinkWarning('Screen OCR component missing')).toContain('完整包并保留 components 文件夹');
    expect(screenLinkWarning('local package not published')).toContain('本地文字识别组件');
    expect(screenLinkWarning('Conversion timeout')).toContain('超时');
    expect(screenLinkWarning('Conversion component installation busy')).toContain('正在使用中');
    expect(screenLinkWarning('private/native/path')).toContain('二维码结果仍可使用');
    expect(screenLinkWarning('')).toBe('');
  });

  it('accepts links-only scans and Enter copies the first visible screen link', async () => {
    const test = setup({ scan: vi.fn(async () => ({ ...combined, items: [] })) }); await test.session.scan();
    await test.session.submit();
    expect(test.port.screenAction).toHaveBeenCalledExactlyOnceWith('scan-1', 'screen-first', 'copy');
    expect(test.port.prepare).toHaveBeenCalledTimes(1);
    expect(test.port.action).not.toHaveBeenCalled();
  });

  it('selects screen URLs before QR codes and traverses both groups in visual order', async () => {
    const test = setup({ scan: vi.fn(async () => combined) }); await test.session.scan();
    await test.session.submit(); test.session.move(1); await test.session.submit();
    expect(test.port.screenAction.mock.calls).toEqual([['scan-1', 'screen-first', 'copy'], ['scan-1', 'screen-second', 'copy']]);
    test.session.move(1); await test.session.submit();
    expect(test.port.action).toHaveBeenLastCalledWith('scan-1', 'url', 'copy');
    test.session.move(1); await test.session.submit();
    expect(test.port.action).toHaveBeenLastCalledWith('scan-1', 'text', 'copy');
    test.session.move(1); expect(test.state().selected).toBe(0);
  });

  it('supports explicit screen-link selection/open using only the native session and link IDs', async () => {
    const test = setup({ scan: vi.fn(async () => combined) }); await test.session.scan();
    test.session.select(1); await test.session.action('open');
    expect(test.port.screenAction).toHaveBeenCalledExactlyOnceWith('scan-1', 'screen-second', 'open');
    await test.session.action('copy', 999); expect(test.port.screenAction).toHaveBeenCalledTimes(1);
  });

  it('preserves QR results when screen OCR has a warning and no screen links', async () => {
    const warned: QrScanResult = { ...result, webLinks: [], webLinkWarning: 'Screen OCR unavailable', webLinksLimited: false };
    const test = setup({ scan: vi.fn(async () => warned) }); await test.session.scan(); await test.session.submit();
    expect(test.state().result).toBe(warned);
    expect(test.port.action).toHaveBeenCalledExactlyOnceWith('scan-1', 'url', 'copy');
    expect(test.port.screenAction).not.toHaveBeenCalled();
  });

  it('retains screen links after action failure and permits retry', async () => {
    const test = setup({ scan: vi.fn(async () => combined), screenAction: vi.fn().mockRejectedValueOnce(new Error('clipboard unavailable')).mockResolvedValue(undefined) });
    await test.session.scan(); await test.session.submit();
    expect(test.state().result).toBe(combined); expect(test.state().error).toContain('剪贴板');
    await test.session.submit(); expect(test.state().notice).toBe('链接已复制到剪贴板');
  });

  it('blocks overlapping screen actions and rescans, and ignores canceled action feedback', async () => {
    const reply = deferred<void>(); const test = setup({ scan: vi.fn(async () => combined), screenAction: vi.fn(() => reply.promise) });
    await test.session.scan(); const work = test.session.submit();
    await test.session.action('open', 1); await test.session.action('copy', 2); await test.session.scan();
    expect(test.port.screenAction).toHaveBeenCalledTimes(1); expect(test.port.action).not.toHaveBeenCalled(); expect(test.port.prepare).toHaveBeenCalledTimes(1);
    test.session.cancel(); const state = test.state(); reply.resolve(); await work; expect(test.state()).toBe(state);
  });

  it('discards late screen links after the scan is canceled', async () => {
    const reply = deferred<QrScanResult>(); const test = setup({ scan: vi.fn(() => reply.promise) });
    const work = test.session.scan(); await Promise.resolve(); test.session.cancel(); const state = test.state();
    reply.resolve(combined); await work; expect(test.state()).toBe(state); expect(test.state().result).toBeNull();
  });
});

describe('legacy QR content link helpers', () => {
  const links: QrLinkResult = { items: [
    { id: 'link-one', url: 'https://example.com/中文', domain: 'example.com' },
    { id: 'link-two', url: 'http://example.org/guide', domain: 'example.org' },
  ], limited: false };

  it('extracts multiple native links without changing original text or the Enter action', async () => {
    const test = setup({ extract: vi.fn(async () => links) }); await test.session.scan();
    await test.session.extract(1);
    expect(test.port.extract).toHaveBeenCalledExactlyOnceWith('scan-1', 'text');
    expect(test.state().links.text).toBe(links);
    expect(test.state().result).toBe(result);
    await test.session.submit();
    expect(test.port.action).toHaveBeenLastCalledWith('scan-1', 'text', 'copy');
  });

  it('uses only native result/link IDs for explicit link copy and open', async () => {
    const test = setup({ extract: vi.fn(async () => links) }); await test.session.scan(); await test.session.extract(1);
    await test.session.action('copy', 1, 'link-two');
    expect(test.port.action).toHaveBeenLastCalledWith('scan-1', 'text', 'copy', 'link-two');
    await test.session.action('open', 1, 'link-one');
    expect(test.port.action).toHaveBeenLastCalledWith('scan-1', 'text', 'open', 'link-one');
    await test.session.action('open', 1, 'forged');
    expect(test.port.action).toHaveBeenCalledTimes(2);
  });

  it('publishes empty and limited extraction results separately per QR code', async () => {
    const test = setup({ extract: vi.fn().mockResolvedValueOnce({ items: [], limited: false }).mockResolvedValueOnce({ ...links, limited: true }) });
    await test.session.scan(); await test.session.extract(0); await test.session.extract(1);
    expect(test.state().links.url).toEqual({ items: [], limited: false });
    expect(test.state().links.text).toMatchObject({ items: links.items, limited: true });
  });

  it('blocks overlapping extraction, scan and original/link actions', async () => {
    const reply = deferred<QrLinkResult>();
    const test = setup({ extract: vi.fn(() => reply.promise) }); await test.session.scan();
    const work = test.session.extract(1);
    await test.session.extract(0); await test.session.scan(); await test.session.submit(); await test.session.action('open', 0);
    expect(test.port.extract).toHaveBeenCalledTimes(1);
    expect(test.port.prepare).toHaveBeenCalledTimes(1);
    expect(test.port.action).not.toHaveBeenCalled();
    expect(test.state()).toMatchObject({ acting: true, extracting: 'text' });
    reply.resolve(links); await work;
    expect(test.state()).toMatchObject({ acting: false, extracting: null });
  });

  it('preserves QR content on extraction errors and allows extraction/action retry', async () => {
    const test = setup({ extract: vi.fn().mockRejectedValueOnce(new Error('QR link extraction failed')).mockResolvedValue(links), action: vi.fn().mockRejectedValueOnce(new Error('clipboard unavailable')).mockResolvedValue(undefined) });
    await test.session.scan(); await test.session.extract(1);
    expect(test.state().error).toContain('提取'); expect(test.state().result).toBe(result);
    await test.session.extract(1); expect(test.state().error).toBe('');
    await test.session.action('copy', 1, 'link-one'); expect(test.state().error).toContain('剪贴板');
    expect(test.state().links.text).toBe(links);
    await test.session.action('copy', 1, 'link-one'); expect(test.state().notice).toBe('链接已复制到剪贴板');
  });

  it.each([false, true])('discards late extraction after session cancellation (failure=%s)', async failed => {
    const reply = deferred<QrLinkResult>(); const test = setup({ extract: vi.fn(() => reply.promise) });
    await test.session.scan(); const work = test.session.extract(1); test.session.cancel(); const state = test.state();
    if (failed) reply.reject(new Error('QR link extraction failed')); else reply.resolve(links);
    await work; expect(test.state()).toBe(state); expect(test.state().links).toEqual({});
  });

  it('rescan clears extracted results and rejects IDs from the previous extraction', async () => {
    const test = setup({ extract: vi.fn(async () => links) }); await test.session.scan(); await test.session.extract(1);
    await test.session.scan(); expect(test.state().links).toEqual({});
    await test.session.action('copy', 1, 'link-one'); expect(test.port.action).not.toHaveBeenCalled();
  });
});

describe('QR scan session', () => {
  it('waits for explicit submit and blocks duplicate captures', async () => {
    const reply = deferred<QrScanResult>();
    const test = setup({ scan: vi.fn(() => reply.promise) });
    expect(test.port.prepare).not.toHaveBeenCalled();
    const work = test.session.submit();
    await test.session.submit();
    expect(test.port.prepare).toHaveBeenCalledTimes(1);
    expect(test.state().pending).toBe(true);
    reply.resolve(result); await work;
    expect(test.state()).toMatchObject({ pending: false, result, selected: 0 });
  });
  it('closes a native preparation that arrives after synchronous cancellation', async () => {
    const reply = deferred<{ sessionId: string }>();
    const test = setup({ prepare: vi.fn(() => reply.promise) });
    const work = test.session.scan();
    test.session.cancel();
    reply.resolve({ sessionId: 'late' }); await work;
    expect(test.port.close).toHaveBeenCalledWith('late');
    expect(test.port.scan).not.toHaveBeenCalled();
    expect(test.state().result).toBeNull();
  });
  it.each([false, true])('suppresses late scan responses after cancellation (failure=%s)', async failed => {
    const reply = deferred<QrScanResult>();
    const test = setup({ scan: vi.fn(() => reply.promise) });
    const work = test.session.scan(); await Promise.resolve();
    test.session.cancel();
    const state = test.state();
    if (failed) reply.reject(new Error('capture failed')); else reply.resolve(result);
    await work;
    expect(test.port.close).toHaveBeenCalledWith('scan-1');
    expect(test.state()).toBe(state);
  });
  it('copies the selected native ID on Enter and opens only on explicit action', async () => {
    const test = setup(); await test.session.submit();
    expect(test.port.action).not.toHaveBeenCalled();
    test.session.move(1); await test.session.submit();
    expect(test.port.action).toHaveBeenLastCalledWith('scan-1', 'text', 'copy');
    await test.session.action('open');
    expect(test.port.action).toHaveBeenCalledTimes(1);
    test.session.move(-1); await test.session.action('open');
    expect(test.port.action).toHaveBeenLastCalledWith('scan-1', 'url', 'open');
  });
  it('retains results and supports retry when an action fails', async () => {
    const test = setup({ action: vi.fn().mockRejectedValueOnce(new Error('clipboard')).mockResolvedValue(undefined) });
    await test.session.scan(); await test.session.submit();
    expect(test.state().result).toBe(result);
    expect(test.state().error).toContain('剪贴板');
    await test.session.submit();
    expect(test.state()).toMatchObject({ error: '', notice: '已复制到剪贴板' });
  });
  it('closes the previous session before rescanning and displays empty and limit replies', async () => {
    const test = setup({ scan: vi.fn().mockResolvedValueOnce(result).mockResolvedValueOnce({ sessionId: 'scan-1', items: [], limited: true }) });
    await test.session.scan(); await test.session.scan();
    expect(test.port.close).toHaveBeenCalledWith('scan-1');
    expect(test.state().result).toMatchObject({ items: [], limited: true });
  });
  it('does not publish a canceled action or expose raw native errors', async () => {
    const reply = deferred<void>();
    const test = setup({ action: vi.fn(() => reply.promise) });
    await test.session.scan(); const work = test.session.submit();
    test.session.cancel(); const state = test.state(); reply.resolve(); await work;
    expect(test.state()).toBe(state);
    expect(qrScanError('<private path or content>')).not.toContain('private');
  });
});

describe('QR browser preview boundary', () => {
  it('defaults off and only exposes synthetic session-bound results', async () => {
    const original = await call<Settings>('get_settings');
    expect(original).toMatchObject({ qrScanEnabled: false, qrScanShortcut: '' });
    await expect(call('prepare_qr_scan')).rejects.toThrow('disabled');
    try {
      await call('update_settings', { patch: { qrScanEnabled: true } });
      const { sessionId } = await call<{ sessionId: string }>('prepare_qr_scan');
      const reply = await call<QrScanResult>('scan_qr_screen', { sessionId });
      expect(reply.items).toHaveLength(2);
      expect(reply.items[0].text).toContain('example.com');
      expect(reply.items[1].text).toContain('合成样例');
      expect(reply.webLinks).toHaveLength(2);
      expect(reply.webLinks?.every(link => !reply.items.some(item => item.text.includes(link.url)))).toBe(true);
      await expect(call('screen_web_link_action', { sessionId, linkId: reply.webLinks![0].id, action: 'copy' })).resolves.toBeUndefined();
      await expect(call('screen_web_link_action', { sessionId, linkId: reply.webLinks![1].id, action: 'open' })).resolves.toBeUndefined();
      await expect(call('screen_web_link_action', { sessionId, linkId: 'forged', action: 'open', url: 'https://untrusted.invalid' })).rejects.toThrow('stale');
      const links = await call<QrLinkResult>('extract_qr_links', { sessionId, resultId: reply.items[1].id });
      expect(links.items).toHaveLength(2);
      await expect(call('extract_qr_links', { sessionId, resultId: 'forged' })).rejects.toThrow('stale');
      await expect(call('qr_scan_action', { sessionId, resultId: reply.items[1].id, linkId: links.items[1].id, action: 'open' })).resolves.toBeUndefined();
      await expect(call('qr_scan_action', { sessionId, resultId: reply.items[1].id, linkId: 'forged', action: 'copy', url: 'https://untrusted.invalid' })).rejects.toThrow('stale');
      await expect(call('qr_scan_action', { sessionId, resultId: reply.items[0].id, action: 'open' })).resolves.toBeUndefined();
      await expect(call('qr_scan_action', { sessionId, resultId: reply.items[1].id, action: 'open' })).rejects.toThrow('invalid');
      await expect(call('qr_scan_action', { sessionId, resultId: 'forged', action: 'copy', text: 'untrusted' })).rejects.toThrow('stale');
      await call('close_qr_scan', { sessionId });
      await expect(call('scan_qr_screen', { sessionId })).rejects.toThrow('stale');
      await expect(call('screen_web_link_action', { sessionId, linkId: reply.webLinks![0].id, action: 'copy' })).rejects.toThrow('stale');
      const pending = await call<{ sessionId: string }>('prepare_qr_scan');
      await call('update_settings', { patch: { qrScanEnabled: false } });
      await expect(call('scan_qr_screen', pending)).rejects.toThrow('disabled');
    } finally { await call('update_settings', { patch: original }); }
  });
});
