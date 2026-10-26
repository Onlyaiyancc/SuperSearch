import { describe, expect, it } from 'vitest';
import { parsePreviewPage, previewZoomStep, previewReadingKey, previewError } from '../src/file-preview';

describe('preview reading controls', () => {
  it('accepts only whole page numbers inside the document and returns zero-based indexes', () => {
    expect(parsePreviewPage(' 12 ', 12)).toBe(11);
    expect(parsePreviewPage('01', 12)).toBe(0);
    for (const input of ['', '0', '-1', '13', '1.5', '1e1', '+2', 'Infinity', '9007199254740993']) expect(parsePreviewPage(input, 12)).toBeNull();
    expect(parsePreviewPage('1', 0)).toBeNull();
  });
  it('steps from fitted zoom in the intended direction and stays within 10–400%', () => {
    expect(previewZoomStep(0.21, 1)).toBe(0.25);
    expect(previewZoomStep(0.21, -1)).toBe(0.1);
    expect(previewZoomStep(1, 1)).toBe(1.25);
    expect(previewZoomStep(1.25, -1)).toBe(1);
    expect(previewZoomStep(4, 1)).toBe(4);
    expect(previewZoomStep(0.1, -1)).toBe(0.1);
  });
  it('routes navigation only when appropriate without taking editing, controls, IME or scrolling keys', () => {
    const context = { kind: 'pdf', blocked: false, fit: true, previous: true, next: true };
    expect(previewReadingKey({ key: 'ArrowRight' }, context)).toBe('next');
    expect(previewReadingKey({ key: 'PageUp' }, context)).toBe('previous');
    expect(previewReadingKey({ key: 'ArrowRight' }, { ...context, fit: false })).toBeNull();
    expect(previewReadingKey({ key: 'PageDown' }, { ...context, kind: 'text' })).toBe('next');
    expect(previewReadingKey({ key: 'ArrowRight' }, { ...context, kind: 'text' })).toBeNull();
    expect(previewReadingKey({ key: 'PageDown' }, { ...context, next: false })).toBeNull();
    for (const extra of [{ ctrlKey: true }, { altKey: true }, { metaKey: true }, { shiftKey: true }, { isComposing: true }, { keyCode: 229 }, { repeat: true }, { defaultPrevented: true }]) expect(previewReadingKey({ key: 'PageDown', ...extra }, context)).toBeNull();
    expect(previewReadingKey({ key: 'PageDown' }, { ...context, blocked: true })).toBeNull();
    expect(previewReadingKey({ key: '+', ctrlKey: true }, context)).toBe('zoomIn');
    expect(previewReadingKey({ key: '-', ctrlKey: true }, context)).toBe('zoomOut');
    expect(previewReadingKey({ key: '0', ctrlKey: true }, context)).toBe('fit');
    expect(previewReadingKey({ key: '+', ctrlKey: true, shiftKey: true }, context)).toBe('zoomIn');
    expect(previewReadingKey({ key: '+', ctrlKey: true }, { ...context, kind: 'text' })).toBeNull();
  });
  it('recognizes native Office errors regardless of capitalization', () => {
    expect(previewError('Office XML 编码不受支持')).toContain('Office 文档');
  });
});
