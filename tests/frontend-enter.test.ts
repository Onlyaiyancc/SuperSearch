import { describe, expect, it } from 'vitest';
import { enterAction } from '../src/search-state';

const settled = { query: '年度报告', busy: false, imeBlocked: false, pending: false, complete: true, hasMatches: false, available: false };

describe('query Enter action', () => {
  it('opens Bing only after a complete search has no local matches', () => {
    expect(enterAction({}, settled)).toBe('web');
    expect(enterAction({}, { ...settled, pending: true })).toBe(null);
    expect(enterAction({}, { ...settled, complete: false })).toBe(null);
    expect(enterAction({}, { ...settled, hasMatches: true, available: false })).toBe(null);
    expect(enterAction({}, { ...settled, hasMatches: true, available: true })).toBe('open');
  });
  it('keeps a directory search local on plain Enter while preserving explicit Shift+Enter', () => {
    for (const query of ['项目/报告', 'D:/项目/报告', 'D:\\项目 报告', '"D:\\Work Files" report', 'report in:"D:\\Work"']) {
      expect(enterAction({}, { ...settled, query })).toBe(null);
      expect(enterAction({ shiftKey: true }, { ...settled, query })).toBe('web');
      expect(enterAction({}, { ...settled, query, hasMatches: true, available: true })).toBe('open');
    }
  });
  it('allows Shift+Enter to force Bing through pending searches and local matches', () => {
    expect(enterAction({ shiftKey: true }, { ...settled, pending: true, complete: false })).toBe('web');
    expect(enterAction({ shiftKey: true }, { ...settled, hasMatches: true, available: true })).toBe('web');
    expect(enterAction({ shiftKey: true }, { ...settled, query: 'in:"invalid', complete: true })).toBe('web');
  });
  it('reserves Ctrl+Enter for revealing an available local result', () => {
    expect(enterAction({ ctrlKey: true }, settled)).toBe(null);
    expect(enterAction({ ctrlKey: true }, { ...settled, hasMatches: true, available: true })).toBe('reveal');
    expect(enterAction({ ctrlKey: true, shiftKey: true }, { ...settled, hasMatches: true, available: true })).toBe(null);
  });
  it('never launches for blank text, busy state, composition, or key repeat', () => {
    for (const event of [{}, { shiftKey: true }, { ctrlKey: true }]) {
      expect(enterAction(event, { ...settled, query: ' \t ' })).toBe(null);
      expect(enterAction(event, { ...settled, busy: true })).toBe(null);
      expect(enterAction(event, { ...settled, imeBlocked: true })).toBe(null);
      expect(enterAction({ ...event, repeat: true }, settled)).toBe(null);
    }
  });
  it('does not turn Alt or Meta modified Enter into a search action', () => {
    expect(enterAction({ altKey: true }, settled)).toBe(null);
    expect(enterAction({ metaKey: true }, settled)).toBe(null);
    expect(enterAction({ shiftKey: true, altKey: true }, settled)).toBe(null);
    expect(enterAction({ shiftKey: true, metaKey: true }, settled)).toBe(null);
  });
});
