import { describe, expect, it } from 'vitest';
import { parseReminderQuery, remainingTime } from '../src/reminders';
import { previewShortcut, markdownHtml, imagePreviewHeight, createPreviewSession, formatPreviewBytes } from '../src/file-preview';

describe('reminder commands', () => {
  it('recognizes explicit commands without taking ordinary filenames', () => {
    expect(parseReminderQuery('提醒')?.seconds).toBeNull();
    expect(parseReminderQuery('倒计时')?.seconds).toBeNull();
    for (const value of ['提醒事项.txt', '倒计时设计.md', 'hello 提醒', 'timer.txt', 'remind.md', 'reminder.txt', 'countdown.pdf', 'remindme', 'timer30s', 'countdown30s']) expect(parseReminderQuery(value)).toBeNull();
  });
  it.each(['提醒', '倒计时', 'remind', 'REMIND', 'reminder', 'timer', 'TIMER', 'countdown', 'COUNTDOWN'])('supports Chinese and English command %s', command => {
    expect(parseReminderQuery(` ${command} `)).toEqual({ seconds: null, title: '', error: '' });
    expect(parseReminderQuery(`${command}\t20m 休息`)).toEqual({ seconds: 1200, title: '休息', error: '' });
    expect(parseReminderQuery(`${command} 30秒`)).toEqual({ seconds: 30, title: '倒计时结束', error: '' });
    expect(parseReminderQuery(`${command} 8d`)?.error).toBeTruthy();
  });
  it('parses bounded compound durations and optional title', () => {
    expect(parseReminderQuery('提醒 1小时30分钟10秒 休息')).toEqual({ seconds: 5410, title: '休息', error: '' });
    expect(parseReminderQuery('倒计时 30秒')).toEqual({ seconds: 30, title: '倒计时结束', error: '' });
    expect(parseReminderQuery('提醒 168小时 截止')?.error).toBe('');
  });
  it('accepts English short and long units, days, case and mixed combinations', () => {
    for (const [duration, seconds] of [['30s', 30], ['20m', 1200], ['2h', 7200], ['1d', 86400], ['1H30M', 5400], ['1d 2h 3m 4s', 93784], ['1小时30m', 5400], ['10sec', 10], ['5mins', 300], ['2hours', 7200], ['7days', 604800], ['1天', 86400]] as const) {
      expect(parseReminderQuery(`提醒 ${duration} 休息`)).toEqual({ seconds, title: '休息', error: '' });
    }
    expect(parseReminderQuery('倒计时 30 s')?.seconds).toBe(30);
    expect(parseReminderQuery('提醒 5m 10s后开会')).toEqual({ seconds: 300, title: '10s后开会', error: '' });
    for (const duration of ['8d', '0s', '-2m', '1.5h', '1ms', '3months', '30seconds.txt', '1hxyz']) expect(parseReminderQuery(`提醒 ${duration}`)?.error).toBeTruthy();
  });
  it('rejects invalid duration and excessive titles before native submission', () => {
    for (const text of ['0秒', '169小时', '1.5分钟', '-2秒', '明天', '30秒 ' + '字'.repeat(101)]) {
      expect(parseReminderQuery('提醒 ' + text)?.error).toBeTruthy();
    }
  });
  it('renders remaining time from the deadline without accumulating ticks', () => {
    expect(remainingTime(1000, 1001)).toBe('即将提醒');
    expect(remainingTime(90500, 0)).toBe('01:31');
    expect(remainingTime(3600000, 0)).toBe('1:00:00');
  });
});

describe('file preview safety and keyboard routing', () => {
  it('formats known preview sizes without hiding small files as zero MiB', () => {
    expect(formatPreviewBytes(0)).toBe('0 B');
    expect(formatPreviewBytes(32)).toBe('32 B');
    expect(formatPreviewBytes(1024)).toBe('1.0 KiB');
    expect(formatPreviewBytes(1536)).toBe('1.5 KiB');
    expect(formatPreviewBytes(1048576)).toBe('1.0 MiB');
    expect(formatPreviewBytes(1073741824)).toBe('1024.0 MiB');
    for (const value of [undefined, -1, NaN, Infinity]) expect(formatPreviewBytes(value)).toBe('');
  });
  it('ignores late replies after close and releases the native session immediately', async () => {
    let resolve!: (value: { kind: string; text: string; mime: string; data: string }) => void;
    let closed = 0, changes = 0;
    const session = createPreviewSession(() => new Promise(done => { resolve = done; }), () => { closed++; }, () => { changes++; });
    const pending = session.load();
    expect(await session.load()).toBe(false);
    session.dispose();
    expect(closed).toBe(1);
    resolve({ kind: 'text', text: 'late', mime: '', data: '' });
    expect(await pending).toBe(false);
    expect(session.state.content).toBeNull();
    expect(changes).toBe(1);
    session.dispose();
    expect(closed).toBe(1);
  });
  it('retains the displayed page after failed navigation and allows retry', async () => {
    const pages: number[] = [];
    const session = createPreviewSession(async ({ page = 0 }) => {
      pages.push(page);
      if (pages.length === 2) throw new Error('Preview PDF busy');
      return { kind: 'pdf', text: '', mime: 'image/png', data: 'AA==', pageIndex: page, pageCount: 2 };
    }, () => {}, () => {});
    expect(await session.load()).toBe(true);
    expect(await session.load({ page: 1 })).toBe(false);
    expect(session.state.content?.pageIndex).toBe(0);
    expect(session.state.error).toBeTruthy();
    expect(await session.load({ page: 1 })).toBe(true);
    expect(session.state.content?.pageIndex).toBe(1);
    expect(session.state.error).toBe('');
  });
  it('fits small images without upscaling and caps wide or tall images', () => {
    expect(imagePreviewHeight(140, 120, 600, 200)).toBe(320);
    expect(imagePreviewHeight(1200, 400, 600, 200)).toBe(400);
    expect(imagePreviewHeight(300, 2000, 600, 200)).toBe(620);
    expect(imagePreviewHeight(1, 1, 600, 200)).toBe(220);
    expect(imagePreviewHeight(1200, 400, 250, 200)).toBe(284);
  });
  it('requires explicit selection and an unmodified space in search input', () => {
    const conditions = { enabled: true, established: true, inputFocused: true, available: true, kind: 'file', pending: false };
    expect(previewShortcut({ key: ' ' }, conditions)).toBe(true);
    expect(previewShortcut({ key: ' ' }, { ...conditions, established: false })).toBe(false);
    expect(previewShortcut({ key: ' ' }, { ...conditions, enabled: false })).toBe(false);
    expect(previewShortcut({ key: ' ' }, { ...conditions, inputFocused: false })).toBe(false);
    expect(previewShortcut({ key: ' ' }, { ...conditions, kind: 'folder' })).toBe(false);
    expect(previewShortcut({ key: ' ', ctrlKey: true }, conditions)).toBe(false);
  });
  it('renders Markdown structure but never active HTML or resource URLs', () => {
    const html = markdownHtml('# Title\n\n**bold** [click](javascript:alert%281%29) ![remote](https://host/x.png)\n\n<script>alert(1)</script>\n<img src=x onerror=alert(1)>');
    expect(html).toContain('<h1>Title</h1>');
    expect(html).toContain('<strong>bold</strong>');
    expect(html).not.toMatch(/<(script|img|iframe)|href=|src=|onerror=/i);
    expect(html).not.toContain('javascript:');
  });
});
