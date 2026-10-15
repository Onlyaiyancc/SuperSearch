import type { FolderCompletion } from './types';

export function scopeDescription(directory?: string | null) {
  if (!directory) return null;
  const absolute = /^[a-z]:[\\/]/i.test(directory) || /^[\\/]{2}/.test(directory);
  return { path: absolute ? directory : directory.replace(/^[\\/]+/, ''), detail: absolute ? '包含子目录' : '同名及子目录' };
}

export function subtitleParts(subtitle: string, title: string) {
  if (!/[\\/]/.test(subtitle) || !subtitle.endsWith(title)) return { parent: '', tail: subtitle };
  const boundary = subtitle.length - title.length;
  let parent = subtitle.slice(0, boundary);
  if (!/^[a-z]:[\\/]$/i.test(parent)) parent = parent.replace(/[\\/]$/, '');
  return { parent, tail: subtitle.slice(parent.length) };
}

export function highlightSegments(title: string, query: string, directory?: string | null, parsedTerms?: string[]) {
  // 目录由后端解析；这里只移除已知范围，关键词按字面匹配并始终作为文本节点渲染。
  let terms = query.trim();
  if (directory) {
    const normalized = terms.replaceAll('/', '\\');
    const target = directory.replaceAll('/', '\\').replace(/^\\(?!\\)/, '').replace(/\\$/, '');
    const offset = normalized.toLocaleLowerCase().indexOf(target.toLocaleLowerCase());
    if (offset >= 0) terms = terms.slice(0, offset) + terms.slice(offset + target.length).replace(/^[\\/]/, '');
  }
  const words = parsedTerms ?? (terms.match(/"[^"]*"|\S+/g) ?? []).map(term => term.replace(/^"|"$/g, ''))
    .filter(term => !/^(?:ext|in):/i.test(term) && !/[\\/]/.test(term))
    .map(term => term.replace(/^(?:app|file|folder):/i, '')).filter(Boolean);
  const matches = Array<boolean>(title.length).fill(false);
  for (const word of words) {
    // 转义正则元字符；直接使用原字符串索引，避免大小写折叠改变 Unicode 长度。
    const escaped = word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    for (const match of title.matchAll(new RegExp(escaped, 'giu'))) matches.fill(true, match.index, match.index + match[0].length);
  }
  const segments: { text: string; match: boolean }[] = [];
  for (let index = 0; index < title.length; index++) {
    const last = segments.at(-1);
    if (last?.match === matches[index]) last.text += title[index];
    else segments.push({ text: title[index], match: matches[index] });
  }
  return segments;
}

export function createCompletionCycle() {
  let candidates: FolderCompletion[] = [];
  let index = -1;
  let cycling = false;
  return {
    update(snapshotId: string, currentId: string, incoming: FolderCompletion[]) {
      if (snapshotId !== currentId || cycling) return;
      candidates = incoming.slice(0, 5);
      index = -1;
    },
    reset() { candidates = []; index = -1; cycling = false; },
    next(event: { inputFocused: boolean; composing?: boolean; ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean; metaKey?: boolean }) {
      if (!event.inputFocused || event.composing || event.ctrlKey || event.altKey || event.metaKey || !candidates.length) return null;
      index = index < 0 ? (event.shiftKey ? candidates.length - 1 : 0) : (index + (event.shiftKey ? candidates.length - 1 : 1)) % candidates.length;
      cycling = true;
      return candidates[index].query;
    },
  };
}
