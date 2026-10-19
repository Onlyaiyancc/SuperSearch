import { describe, expect, it } from 'vitest';
import { captureShortcut, parseAliases } from '../src/settings-input';

describe('shortcut recorder', () => {
  it.each([
    [{ key: ' ', altKey: true }, 'Alt+Space'],
    [{ key: 'k', ctrlKey: true, shiftKey: true }, 'Control+Shift+K'],
    [{ key: 'e', metaKey: true }, 'Super+E'],
    [{ key: '7', ctrlKey: true }, 'Control+7'],
    [{ key: 'F12', altKey: true }, 'Alt+F12'],
    [{ key: 'ArrowDown', ctrlKey: true }, 'Control+ArrowDown'],
    [{ key: 'Home', ctrlKey: true, altKey: true, shiftKey: true, metaKey: true }, 'Control+Alt+Shift+Super+Home'],
  ])('serializes a supported Windows chord %#', (event, shortcut) => {
    expect(captureShortcut(event)).toEqual({ kind: 'capture', shortcut });
  });

  it('leaves Tab and Shift+Tab available for focus navigation', () => {
    expect(captureShortcut({ key: 'Tab' })).toEqual({ kind: 'navigate' });
    expect(captureShortcut({ key: 'Tab', shiftKey: true })).toEqual({ kind: 'navigate' });
  });

  it('exits on Escape without providing a replacement shortcut', () => {
    expect(captureShortcut({ key: 'Escape' })).toEqual({ kind: 'cancel' });
  });

  it.each(['Alt', 'Control', 'Shift', 'Meta', 'OS'])('preserves the chord on modifier-only %s', key => {
    expect(captureShortcut({ key, ctrlKey: true })).toEqual({ kind: 'ignore' });
  });

  it.each([
    { key: 'k', ctrlKey: true, repeat: true },
    { key: 'k', ctrlKey: true, isComposing: true },
    { key: 'k', ctrlKey: true, keyCode: 229 },
  ])('preserves the chord during repeat or composition %#', event => {
    expect(captureShortcut(event)).toEqual({ kind: 'ignore' });
  });

  it('explains that a bare key requires modifiers', () => {
    expect(captureShortcut({ key: 'k' })).toEqual({ kind: 'error', error: 'modifier' });
  });

  it('does not mistake AltGraph text input for a Control+Alt shortcut', () => {
    expect(captureShortcut({ key: 'a', ctrlKey: true, altKey: true, getModifierState: key => key === 'AltGraph' })).toEqual({ kind: 'error', error: 'unsupported' });
  });

  it.each([
    [{ key: '!', code: 'Digit1', ctrlKey: true, shiftKey: true }, 'Control+Shift+1'],
    [{ key: '1', code: 'Numpad1', ctrlKey: true }, 'Control+Numpad1'],
    [{ key: 'Enter', code: 'NumpadEnter', ctrlKey: true }, 'Control+NumpadEnter'],
    [{ key: '+', code: 'NumpadAdd', ctrlKey: true }, 'Control+NumpadAdd'],
    [{ key: '.', code: 'NumpadDecimal', ctrlKey: true }, 'Control+NumpadDecimal'],
    [{ key: 'End', code: 'Numpad1', ctrlKey: true }, 'Control+Numpad1'],
  ])('preserves the physical alphanumeric or numpad key %#', (event, shortcut) => {
    expect(captureShortcut(event)).toEqual({ kind: 'capture', shortcut });
  });

  it('uses the Latin key label on non-QWERTY layouts for Windows virtual-key registration', () => {
    expect(captureShortcut({ key: 'a', code: 'KeyQ', ctrlKey: true })).toEqual({ kind: 'capture', shortcut: 'Control+A' });
  });

  it('rejects a non-Latin label instead of guessing its Windows virtual key from a physical code', () => {
    expect(captureShortcut({ key: 'й', code: 'KeyQ', ctrlKey: true })).toEqual({ kind: 'error', error: 'unsupported' });
  });

  it('rejects an unhandled numpad key instead of recording its main-key equivalent', () => {
    expect(captureShortcut({ key: 'Enter', code: 'NumpadUnsupported', ctrlKey: true })).toEqual({ kind: 'error', error: 'unsupported' });
  });

  it.each([
    { key: '!', code: 'Digit1', ctrlKey: true, isComposing: true },
    { key: '1', code: 'Numpad1', ctrlKey: true, repeat: true },
    { key: 'q', code: 'KeyQ', ctrlKey: true, keyCode: 229 },
  ])('does not use physical codes to bypass composition or repeat protection %#', event => {
    expect(captureShortcut(event)).toEqual({ kind: 'ignore' });
  });

  it.each(['Dead', 'Process', 'Unidentified'])('does not use a physical letter code to capture %s', key => {
    expect(captureShortcut({ key, code: 'KeyQ', ctrlKey: true })).toEqual({ kind: 'error', error: 'unsupported' });
  });

  it.each(['Dead', 'Process', 'Unidentified', '中', '+', ',', 'F25', 'AudioVolumeUp'])('rejects unsupported key %s', key => {
    expect(captureShortcut({ key, ctrlKey: true })).toEqual({ kind: 'error', error: 'unsupported' });
  });
});

describe('application alias input', () => {
  it('preserves unchanged legacy aliases containing either comma style', () => {
    const original = ['Design, Tools', '工作，应用', 'Editor'];
    expect(parseAliases(original.join(', '), original)).toEqual({ aliases: original, error: '' });
  });

  it('parses changed drafts and allows explicitly clearing existing aliases', () => {
    const original = ['Design, Tools'];
    expect(parseAliases('new，editor', original)).toEqual({ aliases: ['new', 'editor'], error: '' });
    expect(parseAliases('', original)).toEqual({ aliases: [], error: '' });
  });

  it('splits both comma styles, trims labels, and discards blanks', () => {
    expect(parseAliases(' code， editor, , 中文， ')).toEqual({ aliases: ['code', 'editor', '中文'], error: '' });
  });

  it('deduplicates compatibility and case equivalents while retaining the first original label', () => {
    expect(parseAliases('ＶＳＣｏｄｅ, vscode，VSCODE, 微信,微信')).toEqual({ aliases: ['ＶＳＣｏｄｅ', '微信'], error: '' });
  });

  it('allows clearing aliases', () => {
    expect(parseAliases(' ， , ')).toEqual({ aliases: [], error: '' });
  });

  it('applies the alias count limit after deduplication', () => {
    expect(parseAliases(Array(20).fill('same').join(',')).error).toBe('');
    expect(parseAliases(Array.from({ length: 16 }, (_, i) => `alias${i}`).join(',')).error).toBe('');
    expect(parseAliases(Array.from({ length: 17 }, (_, i) => `alias${i}`).join(',')).error).toBe('count');
  });

  it('counts Unicode code points for the 128-character limit', () => {
    expect(parseAliases('😀'.repeat(128)).error).toBe('');
    expect(parseAliases('😀'.repeat(129)).error).toBe('length');
  });

  it.each(['one\ntwo', 'one\ttwo', 'one\u0000two', 'one\u007ftwo', 'one\u0085two'])('rejects control characters %#', value => {
    expect(parseAliases(value).error).toBe('control');
  });

  it('validates duplicate labels before discarding them', () => {
    const valid = 'A'.repeat(128);
    expect(parseAliases(`${valid},${valid}A`).error).toBe('length');
  });
});
