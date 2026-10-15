import { describe, expect, it } from 'vitest';
import { highlightSegments, scopeDescription, subtitleParts, createCompletionCycle } from '../src/search-presentation';
import { previewSnapshot } from '../src/bridge';

describe('search presentation', () => {
  it('uses backend terms even when the displayed absolute scope differs from input', () => {
    expect(highlightSegments('预算报告.pdf', '示例项目/报告/预算', 'D:\\项目\\报告', ['预算'])).toEqual([{ text: '预算', match: true }, { text: '报告.pdf', match: false }]);
  });
  it('offers clearly labeled directory fixtures only through the reserved demo prefix', () => {
    const suggestions = previewSnapshot('demo', '示例项目/报', 'all');
    expect(suggestions.directory).toBe('\\示例项目');
    expect(suggestions.folderCompletions?.length).toBe(2);
    expect(suggestions.folderCompletions?.every(folder => folder.path.includes('预览样例') && folder.query.startsWith('示例项目/'))).toBe(true);
    const matches = previewSnapshot('demo2', '示例项目/报告/预算', 'all');
    expect(matches.items.length).toBeGreaterThan(0);
    expect(matches.items.every(item => item.subtitle.includes('预览样例'))).toBe(true);
    expect(() => previewSnapshot('real', 'D:\\项目\\报告', 'all')).toThrow('预览不读取本地文件夹');
  });
  it('distinguishes an absolute directory from a named folder component', () => {
    expect(scopeDescription('D:\\项目')).toEqual({ path: 'D:\\项目', detail: '包含子目录' });
    expect(scopeDescription('\\项目')).toEqual({ path: '项目', detail: '同名及子目录' });
    expect(scopeDescription('\\\\server\\share')).toEqual({ path: '\\\\server\\share', detail: '包含子目录' });
    expect(scopeDescription(null)).toBeNull();
  });
  it('segments literal special characters and multilingual keywords without HTML', () => {
    expect(highlightSegments('<报告[1]>.pdf', '报告[1]')).toEqual([
      { text: '<', match: false }, { text: '报告[1]', match: true }, { text: '>.pdf', match: false },
    ]);
    expect(highlightSegments('Budget预算.PDF', 'budget 预算 ext:pdf')).toEqual([
      { text: 'Budget预算', match: true }, { text: '.PDF', match: false },
    ]);
    expect(highlightSegments('报告.pdf', 'D:\\项目\\报告', 'D:\\项目')).toEqual([
      { text: '报告', match: true }, { text: '.pdf', match: false },
    ]);
    expect(highlightSegments('report.pdf', 'D:\\项目\\', 'D:\\项目')).toEqual([{ text: 'report.pdf', match: false }]);
    expect(highlightSegments('İ预算.pdf', '预算')).toEqual([{ text: 'İ', match: false }, { text: '预算', match: true }, { text: '.pdf', match: false }]);
  });
  it('emphasizes the actual parent without losing full path text', () => {
    expect(subtitleParts('D:\\项目\\报告.pdf', '报告.pdf')).toEqual({ parent: 'D:\\项目', tail: '\\报告.pdf' });
    expect(subtitleParts('C:\\报告.pdf', '报告.pdf')).toEqual({ parent: 'C:\\', tail: '报告.pdf' });
    expect(subtitleParts('预览样例 · 应用', 'Terminal')).toEqual({ parent: '', tail: '预览样例 · 应用' });
  });
});

describe('folder completion cycling', () => {
  const folders = [{ title: '甲', path: 'D:\\甲', query: 'D:\\甲\\' }, { title: '乙', path: 'D:\\乙', query: 'D:\\乙\\' }];
  it('cycles forwards/backwards through the original current batch while a query runs', () => {
    const cycle = createCompletionCycle();
    cycle.update('q1', 'q1', folders);
    expect(cycle.next({ inputFocused: true })).toBe('D:\\甲\\');
    expect(cycle.next({ inputFocused: true })).toBe('D:\\乙\\');
    expect(cycle.next({ inputFocused: true, shiftKey: true })).toBe('D:\\甲\\');
    expect(cycle.next({ inputFocused: true, shiftKey: true })).toBe('D:\\乙\\');
  });
  it('does not intercept Ctrl+Tab, IME, other focus or empty candidates', () => {
    const cycle = createCompletionCycle();
    cycle.update('q1', 'q1', folders);
    for (const guard of [{ ctrlKey: true }, { composing: true }, { altKey: true }, { metaKey: true }, { inputFocused: false }]) {
      expect(cycle.next({ inputFocused: true, ...guard })).toBeNull();
    }
    expect(cycle.next({ inputFocused: true, shiftKey: true })).toBe('D:\\乙\\');
    cycle.reset();
    expect(cycle.next({ inputFocused: true })).toBeNull();
  });
  it('drops candidates on manual edits and rejects snapshots from earlier requests', () => {
    const cycle = createCompletionCycle();
    cycle.update('q1', 'q1', folders);
    cycle.reset();
    cycle.update('q1', 'q2', folders);
    expect(cycle.next({ inputFocused: true })).toBeNull();
    cycle.update('q2', 'q2', [folders[1]]);
    expect(cycle.next({ inputFocused: true })).toBe('D:\\乙\\');
  });
  it('retains the original cycle when a completion request returns other folders', () => {
    const cycle = createCompletionCycle();
    cycle.update('q1', 'q1', folders);
    cycle.next({ inputFocused: true });
    cycle.update('q2', 'q2', [{ title: '子', path: 'D:\\甲\\子', query: 'D:\\甲\\子\\' }]);
    expect(cycle.next({ inputFocused: true })).toBe('D:\\乙\\');
  });
});
