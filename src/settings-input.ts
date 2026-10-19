type ShortcutInput = Pick<KeyboardEvent, 'key'> & Partial<Pick<KeyboardEvent, 'code' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey' | 'isComposing' | 'repeat' | 'keyCode' | 'getModifierState'>>;
type ShortcutCapture = { kind: 'capture'; shortcut: string } | { kind: 'error'; error: 'modifier' | 'unsupported' } | { kind: 'navigate' | 'cancel' | 'ignore' };
type AliasInput = { aliases: string[]; error: '' | 'count' | 'length' | 'control' };

export function captureShortcut(event: ShortcutInput): ShortcutCapture {
  if (event.key === 'Tab') return { kind: 'navigate' };
  if (event.key === 'Escape') return { kind: 'cancel' };
  if (event.repeat || event.isComposing || event.keyCode === 229 || ['Alt', 'Control', 'Shift', 'Meta', 'OS'].includes(event.key)) return { kind: 'ignore' };
  if (event.getModifierState?.('AltGraph')) return { kind: 'error', error: 'unsupported' };
  const modifiers = [event.ctrlKey && 'Control', event.altKey && 'Alt', event.shiftKey && 'Shift', event.metaKey && 'Super'].filter(Boolean);
  if (!modifiers.length) return { kind: 'error', error: 'modifier' };
  if (['Dead', 'Process', 'Unidentified'].includes(event.key)) return { kind: 'error', error: 'unsupported' };
  const code = event.code ?? '';
  const numpad = /^Numpad([0-9]|Add|Decimal|Divide|Enter|Equal|Multiply|Subtract)$/.test(code);
  if (code.startsWith('Numpad') && !numpad) return { kind: 'error', error: 'unsupported' };
  // 数字行与小键盘保留键位区别；字母沿用布局后的键名，避免将 AZERTY 的 A 注册成 Q。
  const key = numpad ? code : /^Digit[0-9]$/.test(code) ? code.slice(5) : event.key === ' ' ? 'Space' : /^[a-z0-9]$/i.test(event.key) ? event.key.toUpperCase() : event.key;
  // 仅输出原生快捷键解析器支持、且满足后端 ASCII 校验的键名。
  if (!numpad && !/^[A-Z0-9]$/.test(key) && !/^F([1-9]|1[0-9]|2[0-4])$/.test(key) && !['Space', 'Enter', 'Backspace', 'Delete', 'Insert', 'Home', 'End', 'PageUp', 'PageDown', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(key)) return { kind: 'error', error: 'unsupported' };
  return { kind: 'capture', shortcut: [...modifiers, key].join('+') };
}

export function parseAliases(text: string, original: readonly string[] = []): AliasInput {
  const aliases: string[] = [];
  const seen = new Set<string>();
  // 未改动的旧别名按原始分组保存，避免把标签内已有的逗号误当分隔符。
  const labels = text === original.join(', ') ? original : text.split(/[,，]/u);
  for (const raw of labels) {
    if (/[\u0000-\u001f\u007f-\u009f]/u.test(raw)) return { aliases: [], error: 'control' };
    const alias = raw.trim();
    if (!alias) continue;
    if (Array.from(alias).length > 128) return { aliases: [], error: 'length' };
    // 与后端逐字符 NFKC 小写化保持一致，原始标签用于展示。
    const key = Array.from(alias.normalize('NFKC'), character => character.toLowerCase()).join('');
    if (!seen.has(key)) { seen.add(key); aliases.push(alias); }
  }
  return aliases.length > 16 ? { aliases: [], error: 'count' } : { aliases, error: '' };
}
